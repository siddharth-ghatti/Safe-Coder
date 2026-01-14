//! HTTP client for TUI to communicate with the server
//!
//! This module provides an HTTP client abstraction that allows the TUI
//! to communicate with the safe-coder server using the same REST/SSE
//! interface as the desktop app.

use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::server::types::{
    CreateSessionRequest, DoomLoopResponseRequest, SendMessageRequest, SessionResponse, ServerEvent,
};

/// Default server port for TUI
pub const DEFAULT_PORT: u16 = 9876;

/// HTTP client for communicating with the safe-coder server
pub struct SafeCoderClient {
    base_url: String,
    client: Client,
    session_id: Option<String>,
}

impl SafeCoderClient {
    /// Create a new client
    pub fn new(port: u16) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            base_url: format!("http://127.0.0.1:{}", port),
            client,
            session_id: None,
        }
    }

    /// Get the current session ID
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Check if the server is healthy
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Wait for server to be ready (with retries)
    pub async fn wait_for_server(&self, max_attempts: u32) -> Result<()> {
        for attempt in 1..=max_attempts {
            if self.health_check().await? {
                return Ok(());
            }
            if attempt < max_attempts {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        anyhow::bail!("Server did not become ready after {} attempts", max_attempts)
    }

    /// Create a new session
    pub async fn create_session(&mut self, project_path: &str, mode: Option<&str>) -> Result<SessionResponse> {
        let url = format!("{}/api/sessions", self.base_url);
        let request = CreateSessionRequest {
            project_path: project_path.to_string(),
            mode: mode.map(|s| s.to_string()),
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to create session")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to create session: {} - {}", status, text);
        }

        let session: SessionResponse = resp.json().await.context("Failed to parse session response")?;
        self.session_id = Some(session.id.clone());
        Ok(session)
    }

    /// Send a message to the current session
    pub async fn send_message(&self, content: &str) -> Result<()> {
        let session_id = self
            .session_id
            .as_ref()
            .context("No active session")?;

        let url = format!("{}/api/sessions/{}/messages", self.base_url, session_id);
        let request = SendMessageRequest {
            content: content.to_string(),
            attachments: vec![],
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send message")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send message: {} - {}", status, text);
        }

        Ok(())
    }

    /// Send a message with attachments
    pub async fn send_message_with_attachments(
        &self,
        content: &str,
        attachments: Vec<AttachmentInput>,
    ) -> Result<()> {
        let session_id = self
            .session_id
            .as_ref()
            .context("No active session")?;

        let url = format!("{}/api/sessions/{}/messages", self.base_url, session_id);
        let request = SendMessageRequest {
            content: content.to_string(),
            attachments: attachments
                .into_iter()
                .map(|a| crate::server::types::AttachmentDto {
                    path: a.path,
                    content: a.content,
                })
                .collect(),
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send message")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to send message: {} - {}", status, text);
        }

        Ok(())
    }

    /// Set session mode (plan/build)
    pub async fn set_mode(&self, mode: &str) -> Result<()> {
        let session_id = self
            .session_id
            .as_ref()
            .context("No active session")?;

        let url = format!("{}/api/sessions/{}/mode", self.base_url, session_id);

        let resp = self
            .client
            .put(&url)
            .json(&serde_json::json!({ "mode": mode }))
            .send()
            .await
            .context("Failed to set mode")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to set mode: {} - {}", status, text);
        }

        Ok(())
    }

    /// Respond to a doom loop prompt
    pub async fn respond_to_doom_loop(&self, prompt_id: &str, continue_anyway: bool) -> Result<()> {
        let session_id = self
            .session_id
            .as_ref()
            .context("No active session")?;

        let url = format!(
            "{}/api/sessions/{}/doom-loop-response",
            self.base_url, session_id
        );
        let request = DoomLoopResponseRequest {
            prompt_id: prompt_id.to_string(),
            continue_anyway,
        };

        let resp = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to respond to doom loop")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Failed to respond to doom loop: {} - {}", status, text);
        }

        Ok(())
    }

    /// Delete/close the current session
    pub async fn close_session(&mut self) -> Result<()> {
        if let Some(session_id) = self.session_id.take() {
            let url = format!("{}/api/sessions/{}", self.base_url, session_id);
            let _ = self.client.delete(&url).send().await;
        }
        Ok(())
    }

    /// Subscribe to SSE events for the current session
    /// Returns a receiver that yields ServerEvents
    pub async fn subscribe_events(&self) -> Result<mpsc::UnboundedReceiver<ServerEvent>> {
        let session_id = self
            .session_id
            .as_ref()
            .context("No active session")?
            .clone();

        let url = format!("{}/api/sessions/{}/events", self.base_url, session_id);
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn SSE listener task
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = sse_listener(client, url, tx).await {
                tracing::error!("SSE listener error: {}", e);
            }
        });

        Ok(rx)
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Attachment input for sending messages
#[derive(Debug, Clone)]
pub struct AttachmentInput {
    pub path: String,
    pub content: Option<String>,
}

/// SSE listener that parses server-sent events
async fn sse_listener(
    client: Client,
    url: String,
    tx: mpsc::UnboundedSender<ServerEvent>,
) -> Result<()> {
    let resp = client
        .get(&url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .context("Failed to connect to SSE endpoint")?;

    if !resp.status().is_success() {
        anyhow::bail!("SSE connection failed: {}", resp.status());
    }

    let mut buffer = String::new();
    let mut bytes_stream = resp.bytes_stream();

    use futures::StreamExt;
    while let Some(chunk) = bytes_stream.next().await {
        let chunk = chunk.context("Error reading SSE chunk")?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        // Process complete SSE messages (double newline separated)
        while let Some(pos) = buffer.find("\n\n") {
            let message = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            // Parse SSE message
            if let Some(event) = parse_sse_message(&message) {
                if tx.send(event).is_err() {
                    // Receiver dropped, stop listening
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// Parse a single SSE message into a ServerEvent
fn parse_sse_message(message: &str) -> Option<ServerEvent> {
    let mut event_type = None;
    let mut data = None;

    for line in message.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data: ") {
            data = Some(rest.to_string());
        }
    }

    // If we have data, try to parse it based on event type
    let data = data?;

    // For most events, the data is JSON that includes the type
    // Try parsing directly as ServerEvent first
    if let Ok(event) = serde_json::from_str::<ServerEvent>(&data) {
        return Some(event);
    }

    // Fallback: handle simple events
    match event_type.as_deref() {
        Some("connected") => Some(ServerEvent::Connected),
        Some("completed") => Some(ServerEvent::Completed),
        Some("error") => {
            let message = serde_json::from_str::<serde_json::Value>(&data)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| data);
            Some(ServerEvent::Error { message })
        }
        _ => None,
    }
}

/// Server process manager for starting/stopping the local server
pub struct ServerManager {
    process: Option<Child>,
    port: u16,
}

impl ServerManager {
    /// Create a new server manager
    pub fn new(port: u16) -> Self {
        Self {
            process: None,
            port,
        }
    }

    /// Check if server is already running on the port
    pub async fn is_running(&self) -> bool {
        let client = SafeCoderClient::new(self.port);
        client.health_check().await.unwrap_or(false)
    }

    /// Start the server if not already running
    pub async fn ensure_running(&mut self) -> Result<()> {
        // Check if already running
        if self.is_running().await {
            tracing::info!("Server already running on port {}", self.port);
            return Ok(());
        }

        // Start the server
        tracing::info!("Starting server on port {}...", self.port);

        // Get the current executable path
        let exe_path = std::env::current_exe().context("Failed to get current executable")?;

        let child = Command::new(&exe_path)
            .args(["serve", "--port", &self.port.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("Failed to start server process")?;

        self.process = Some(child);

        // Wait for server to be ready
        let client = SafeCoderClient::new(self.port);
        client.wait_for_server(30).await?;

        tracing::info!("Server started successfully");
        Ok(())
    }

    /// Stop the server if we started it
    pub async fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
        }
    }

    /// Get the port
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ServerManager {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            // Try to kill synchronously on drop
            let _ = child.start_kill();
        }
    }
}
