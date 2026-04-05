//! Minimal CDP client over raw WebSocket.
//!
//! Only enables `Runtime` and `Network` domains — deliberately skips `Page.enable`
//! to avoid setting `navigator.webdriver = true` (which triggers Cloudflare detection).

use anyhow::{Context, Result, bail};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tracing::debug;

/// A minimal CDP session attached to a single page target.
pub struct CdpSession {
    next_id: Arc<AtomicU64>,
    tx: mpsc::UnboundedSender<Message>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Channel receiving CDP events (method + params).
    pub events: mpsc::UnboundedReceiver<(String, Value)>,
}

impl CdpSession {
    /// Connect to a Chrome page target's WebSocket endpoint.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .with_context(|| format!("failed to connect to {ws_url}"))?;

        let (mut sink, mut stream) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        let (event_tx, event_rx) = mpsc::unbounded_channel::<(String, Value)>();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_read = Arc::clone(&pending);

        // Writer task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        // Reader task — routes responses to pending requests, events to event channel
        tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(val) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };

                if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                    // Response to a command
                    let mut map = pending_read.lock().await;
                    if let Some(sender) = map.remove(&id) {
                        let _ = sender.send(val);
                    }
                } else if let Some(method) = val.get("method").and_then(|v| v.as_str()) {
                    // Event
                    let params = val.get("params").cloned().unwrap_or(Value::Null);
                    let _ = event_tx.send((method.to_string(), params));
                }
            }
        });

        Ok(Self {
            next_id: Arc::new(AtomicU64::new(1)),
            tx,
            pending,
            events: event_rx,
        })
    }

    /// Send a CDP command and wait for the response.
    pub async fn send(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({ "id": id, "method": method, "params": params });

        let (resp_tx, resp_rx) = oneshot::channel();
        self.pending.lock().await.insert(id, resp_tx);

        self.tx
            .send(Message::Text(msg.to_string().into()))
            .map_err(|_| anyhow::anyhow!("WS send failed"))?;

        let resp = tokio::time::timeout(std::time::Duration::from_secs(30), resp_rx)
            .await
            .context("CDP command timed out")?
            .context("CDP response channel closed")?;

        if let Some(err) = resp.get("error") {
            bail!("CDP error for {method}: {err}");
        }

        Ok(resp.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Evaluate a JS expression and return the result as a string.
    pub async fn evaluate(&self, expression: &str) -> Result<Value> {
        let result = self
            .send(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;

        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// Evaluate JS and return the result as a String.
    pub async fn evaluate_string(&self, expression: &str) -> Result<String> {
        let val = self.evaluate(expression).await?;
        Ok(val.as_str().unwrap_or_default().to_string())
    }

    /// Evaluate JS and return the result as a bool.
    pub async fn evaluate_bool(&self, expression: &str) -> Result<bool> {
        let val = self.evaluate(expression).await?;
        Ok(val.as_bool().unwrap_or(false))
    }

    /// Evaluate JS and return the result as f64.
    pub async fn evaluate_f64(&self, expression: &str) -> Result<f64> {
        let val = self.evaluate(expression).await?;
        Ok(val.as_f64().unwrap_or(0.0))
    }

    /// Get a network response body by request ID.
    pub async fn get_response_body(&self, request_id: &str) -> Result<Vec<u8>> {
        let result = self
            .send(
                "Network.getResponseBody",
                json!({ "requestId": request_id }),
            )
            .await?;

        let body = result
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let base64_encoded = result
            .get("base64Encoded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if base64_encoded {
            Ok(base64::engine::general_purpose::STANDARD.decode(body)?)
        } else {
            Ok(body.as_bytes().to_vec())
        }
    }

    /// Enable only the CDP domains we need (Runtime + Network). Skips Page.enable.
    pub async fn enable_domains(&self) -> Result<()> {
        self.send("Runtime.enable", json!({})).await?;
        self.send("Network.enable", json!({})).await?;
        debug!("Enabled Runtime + Network domains (Page deliberately skipped)");
        Ok(())
    }

    /// Navigate by evaluating JS — avoids Page.navigate which requires Page domain.
    pub async fn navigate(&self, url: &str) -> Result<()> {
        self.evaluate(&format!("window.location.href = '{url}'"))
            .await?;
        Ok(())
    }
}

/// Discover the WebSocket debug URL for the first page target.
#[derive(Deserialize)]
struct TargetInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
    #[serde(rename = "type")]
    target_type: String,
}

/// Get the WS URL for the first page target from Chrome's debug endpoint.
pub async fn discover_page_ws(port: u16) -> Result<String> {
    let targets: Vec<TargetInfo> = reqwest::get(format!("http://127.0.0.1:{port}/json"))
        .await?
        .json()
        .await?;

    targets
        .into_iter()
        .find(|t| t.target_type == "page")
        .and_then(|t| t.web_socket_debugger_url)
        .context("no page target found")
}
