//! Cross-platform Chrome detection, launch, and CDP connection.
//!
//! Launches a real Chrome instance with `--remote-debugging-port` to avoid
//! Cloudflare bot detection. Connects via CDP for automation.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use tempfile::TempDir;
use tokio::time::{Duration, sleep};
use tracing::{debug, info};

/// Default CDP debugging port.
const DEBUG_PORT: u16 = 9222;

/// A managed Chrome instance with CDP connection.
pub struct Chrome {
    process: Child,
    _profile_dir: TempDir,
    /// The CDP browser handle.
    pub browser: chromiumoxide::Browser,
    _handler: tokio::task::JoinHandle<()>,
}

impl Chrome {
    /// Find Chrome, launch it, and connect via CDP.
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
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("failed to launch Chrome: {}", chrome_path.display()))?;

        info!("Chrome launched (pid {}), waiting for CDP...", process.id());

        let cdp_url = format!("http://127.0.0.1:{DEBUG_PORT}");
        wait_for_cdp(&cdp_url).await?;

        let (browser, mut handler) = chromiumoxide::Browser::connect(&cdp_url)
            .await
            .context("failed to connect to Chrome via CDP")?;

        // Handler is a Stream — drive it in the background
        let _handler = tokio::spawn(async move { while handler.next().await.is_some() {} });

        info!("CDP connected.");

        Ok(Self {
            process,
            _profile_dir: profile_dir,
            browser,
            _handler,
        })
    }

    /// Gracefully shut down Chrome.
    pub fn shutdown(&mut self) {
        debug!("Shutting down Chrome (pid {})...", self.process.id());
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
async fn wait_for_cdp(url: &str) -> Result<()> {
    let version_url = format!("{url}/json/version");

    for attempt in 1..=30 {
        if let Ok(Ok(resp)) =
            tokio::time::timeout(Duration::from_millis(500), reqwest::get(&version_url)).await
        {
            if resp.status().is_success() {
                debug!("CDP ready after {attempt} attempts");
                return Ok(());
            }
        }
        sleep(Duration::from_millis(200)).await;
    }

    bail!("Chrome CDP endpoint did not become available at {url}")
}

/// Detect the Chrome/Chromium binary for the current platform.
fn find_chrome() -> Result<PathBuf> {
    for candidate in chrome_candidates() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    for name in path_candidates() {
        if let Ok(path) = which::which(name) {
            return Ok(path);
        }
    }

    bail!(
        "Chrome not found. Install Google Chrome or Chromium.\n\
         Searched: {:?}",
        chrome_candidates()
    )
}

/// Platform-specific hardcoded Chrome paths.
#[cfg(target_os = "macos")]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
    ]
}

#[cfg(target_os = "windows")]
fn chrome_candidates() -> Vec<PathBuf> {
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    let pf86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();

    let mut paths = vec![
        PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe"),
        PathBuf::from(&pf86).join(r"Google\Chrome\Application\chrome.exe"),
    ];
    if !local.is_empty() {
        paths.push(PathBuf::from(&local).join(r"Google\Chrome\Application\chrome.exe"));
    }
    paths
}

#[cfg(target_os = "linux")]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/bin/google-chrome"),
        PathBuf::from("/usr/bin/google-chrome-stable"),
        PathBuf::from("/usr/bin/chromium-browser"),
        PathBuf::from("/usr/bin/chromium"),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn chrome_candidates() -> Vec<PathBuf> {
    vec![]
}

/// Binary names to search in PATH as fallback.
fn path_candidates() -> &'static [&'static str] {
    &[
        "google-chrome",
        "google-chrome-stable",
        "chromium-browser",
        "chromium",
        "chrome",
    ]
}
