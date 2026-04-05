//! Cross-platform Chrome detection, launch, and CDP connection.

use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use tokio::time::{Duration, sleep};
use tracing::{debug, info};

use crate::cdp::{CdpSession, discover_page_ws};

/// Default CDP debugging port.
const DEBUG_PORT: u16 = 9222;

/// A managed Chrome instance with a CDP session.
pub struct Chrome {
    process: Child,
    _profile_dir: TempDir,
    /// The CDP session attached to the page.
    pub session: CdpSession,
}

impl Chrome {
    /// Find Chrome, launch it, connect via raw CDP WebSocket.
    pub async fn start() -> Result<Self> {
        let chrome_path = find_chrome()?;
        info!("Found Chrome: {}", chrome_path.display());

        let profile_dir = TempDir::new().context("failed to create temp profile dir")?;
        debug!("Chrome profile: {}", profile_dir.path().display());

        let process = Command::new(&chrome_path)
            .arg(format!("--remote-debugging-port={DEBUG_PORT}"))
            .arg(format!("--user-data-dir={}", profile_dir.path().display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-blink-features=AutomationControlled")
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to launch Chrome: {}", chrome_path.display()))?;

        info!("Chrome launched (pid {}), waiting for CDP...", process.id());
        wait_for_cdp(DEBUG_PORT).await?;

        let ws_url = discover_page_ws(DEBUG_PORT).await?;
        debug!("Page WS: {ws_url}");

        let session = CdpSession::connect(&ws_url).await?;
        session.enable_domains().await?;

        info!("CDP connected (raw WS, no Page domain).");

        Ok(Self {
            process,
            _profile_dir: profile_dir,
            session,
        })
    }

    /// Gracefully shut down Chrome.
    pub fn shutdown(&mut self) {
        debug!("Shutting down Chrome...");
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Wait for Chrome's CDP endpoint to accept connections.
async fn wait_for_cdp(port: u16) -> Result<()> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    for attempt in 1..=30 {
        if let Ok(Ok(resp)) =
            tokio::time::timeout(Duration::from_millis(500), reqwest::get(&url)).await
        {
            if resp.status().is_success() {
                debug!("CDP ready after {attempt} attempts");
                return Ok(());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
    bail!("Chrome CDP endpoint not available on port {port}")
}

/// Detect the Chrome/Chromium binary for the current platform.
fn find_chrome() -> Result<PathBuf> {
    for candidate in chrome_candidates() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    for name in PATH_CANDIDATES {
        if let Ok(path) = which::which(name) {
            return Ok(path);
        }
    }
    bail!("Chrome not found. Install Google Chrome or Chromium.")
}

const PATH_CANDIDATES: &[&str] = &[
    "google-chrome",
    "google-chrome-stable",
    "chromium-browser",
    "chromium",
    "chrome",
];

#[cfg(target_os = "macos")]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into(),
        "/Applications/Chromium.app/Contents/MacOS/Chromium".into(),
    ]
}

#[cfg(target_os = "windows")]
fn chrome_candidates() -> Vec<PathBuf> {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    let pf86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let mut v = vec![
        PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe"),
        PathBuf::from(&pf86).join(r"Google\Chrome\Application\chrome.exe"),
    ];
    if !local.is_empty() {
        v.push(PathBuf::from(&local).join(r"Google\Chrome\Application\chrome.exe"));
    }
    v
}

#[cfg(target_os = "linux")]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        "/usr/bin/google-chrome".into(),
        "/usr/bin/google-chrome-stable".into(),
        "/usr/bin/chromium-browser".into(),
        "/usr/bin/chromium".into(),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![]
}
