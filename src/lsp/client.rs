//! LSP Client
//!
//! Handles communication with a single language server via JSON-RPC over stdio.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use super::config::LspServerConfig;
use super::protocol::*;

/// LSP Client for communicating with a language server
pub struct LspClient {
    /// Language identifier (e.g., "rust", "go")
    pub language_id: String,
    /// Server configuration
    config: LspServerConfig,
    /// Child process
    process: Option<Child>,
    /// Request ID counter
    next_id: AtomicU64,
    /// Pending requests waiting for responses
    pending_requests: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<JsonRpcResponse>>>>,
    /// Server capabilities (after initialization)
    capabilities: Option<ServerCapabilities>,
    /// Root URI
    root_uri: Option<String>,
    /// Whether initialized
    initialized: bool,
    /// Diagnostics channel sender
    diagnostics_tx: Option<mpsc::UnboundedSender<(String, Vec<LspDiagnostic>)>>,
    /// Stdout reader thread handle
    reader_handle: Option<std::thread::JoinHandle<()>>,
}

impl LspClient {
    /// Create a new LSP client
    pub fn new(language_id: String, config: LspServerConfig) -> Self {
        Self {
            language_id,
            config,
            process: None,
            next_id: AtomicU64::new(1),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            capabilities: None,
            root_uri: None,
            initialized: false,
            diagnostics_tx: None,
            reader_handle: None,
        }
    }

    /// Check if the language server command is available
    pub fn is_available(&self) -> bool {
        if self.config.command.is_empty() {
            return false;
        }

        // Check if command exists in PATH
        which::which(&self.config.command).is_ok()
    }

    /// Start the language server
    pub fn start(&mut self, root_path: &PathBuf) -> Result<()> {
        if self.process.is_some() {
            return Ok(());
        }

        if !self.is_available() {
            return Err(anyhow::anyhow!(
                "Language server '{}' not found in PATH",
                self.config.command
            ));
        }

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(root_path);

        // Set environment variables
        for (key, value) in &self.config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().context(format!(
            "Failed to start language server: {}",
            self.config.command
        ))?;

        // Set up root URI
        self.root_uri = Some(format!("file://{}", root_path.display()));

        // Take stdout for reading
        let stdout = child.stdout.take().context("Failed to get stdout")?;

        // Store the process
        self.process = Some(child);

        // Start reader thread
        let pending = Arc::clone(&self.pending_requests);
        let (diag_tx, _diag_rx) = mpsc::unbounded_channel();
        self.diagnostics_tx = Some(diag_tx.clone());

        let handle = std::thread::spawn(move || {
            Self::read_messages(stdout, pending, diag_tx);
        });
        self.reader_handle = Some(handle);

        Ok(())
    }

    /// Read messages from server stdout
    fn read_messages(
        stdout: std::process::ChildStdout,
        pending: Arc<Mutex<HashMap<u64, tokio::sync::oneshot::Sender<JsonRpcResponse>>>>,
        diag_tx: mpsc::UnboundedSender<(String, Vec<LspDiagnostic>)>,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut headers = String::new();

        loop {
            headers.clear();

            // Read headers until empty line
            let mut content_length: usize = 0;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return, // EOF
                    Ok(_) => {
                        let line = line.trim();
                        if line.is_empty() {
                            break;
                        }
                        if line.to_lowercase().starts_with("content-length:") {
                            if let Some(len_str) = line.split(':').nth(1) {
                                content_length = len_str.trim().parse().unwrap_or(0);
                            }
                        }
                    }
                    Err(_) => return,
                }
            }

            if content_length == 0 {
                continue;
            }

            // Read content
            let mut content = vec![0u8; content_length];
            if std::io::Read::read_exact(&mut reader, &mut content).is_err() {
                continue;
            }

            let content_str = match String::from_utf8(content) {
                Ok(s) => s,
                Err(_) => continue,
            };

            // Try to parse as response or notification
            if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&content_str) {
                if let Some(id) = response.id {
                    if let Ok(mut pending) = pending.lock() {
                        if let Some(tx) = pending.remove(&id) {
                            let _ = tx.send(response);
                        }
                    }
                }
            } else if let Ok(notification) = serde_json::from_str::<serde_json::Value>(&content_str)
            {
                // Handle notifications
                if let Some(method) = notification.get("method").and_then(|m| m.as_str()) {
                    if method == "textDocument/publishDiagnostics" {
                        if let Some(params) = notification.get("params") {
                            if let Ok(diag_params) =
                                serde_json::from_value::<PublishDiagnosticsParams>(params.clone())
                            {
                                let _ = diag_tx.send((diag_params.uri, diag_params.diagnostics));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Send a request and wait for response
    pub async fn request<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<T> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        // Create response channel
        let (tx, rx) = tokio::sync::oneshot::channel();
        {
            let mut pending = self
                .pending_requests
                .lock()
                .map_err(|e| anyhow::anyhow!("Lock error: {}", e))?;
            pending.insert(id, tx);
        }

        // Send request
        self.send_message(&serde_json::to_string(&request)?)?;

        // Wait for response with timeout
        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .context("Request timed out")?
            .context("Response channel closed")?;

        if let Some(error) = response.error {
            return Err(anyhow::anyhow!(
                "LSP error {}: {}",
                error.code,
                error.message
            ));
        }

        let result = response.result.context("No result in response")?;
        serde_json::from_value(result).context("Failed to parse response")
    }

    /// Send a notification (no response expected)
    pub fn notify(&mut self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let notification = JsonRpcNotification::new(method, params);
        self.send_message(&serde_json::to_string(&notification)?)
    }

    /// Send a raw message to the server
    fn send_message(&mut self, content: &str) -> Result<()> {
        let process = self.process.as_mut().context("Server not started")?;
        let stdin = process.stdin.as_mut().context("No stdin")?;

        let message = format!("Content-Length: {}\r\n\r\n{}", content.len(), content);
        stdin.write_all(message.as_bytes())?;
        stdin.flush()?;

        Ok(())
    }

    /// Initialize the server
    pub async fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: self.root_uri.clone(),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    publish_diagnostics: Some(PublishDiagnosticsCapabilities {
                        related_information: Some(true),
                    }),
                    hover: Some(HoverCapabilities {
                        content_format: Some(vec!["markdown".to_string(), "plaintext".to_string()]),
                    }),
                }),
            },
            initialization_options: self.config.initialization_options.clone(),
        };

        let result: InitializeResult = self
            .request("initialize", Some(serde_json::to_value(params)?))
            .await?;

        self.capabilities = Some(result.capabilities);

        // Send initialized notification
        self.notify("initialized", Some(serde_json::json!({})))?;
        self.initialized = true;

        Ok(())
    }

    /// Notify server that a document was opened
    pub fn did_open(
        &mut self,
        uri: &str,
        language_id: &str,
        version: i32,
        text: &str,
    ) -> Result<()> {
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.to_string(),
                language_id: language_id.to_string(),
                version,
                text: text.to_string(),
            },
        };

        self.notify("textDocument/didOpen", Some(serde_json::to_value(params)?))
    }

    /// Notify server that a document was changed
    pub fn did_change(&mut self, uri: &str, version: i32, text: &str) -> Result<()> {
        let params = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.to_string(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        };

        self.notify(
            "textDocument/didChange",
            Some(serde_json::to_value(params)?),
        )
    }

    /// Notify server that a document was closed
    pub fn did_close(&mut self, uri: &str) -> Result<()> {
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: uri.to_string(),
            },
        };

        self.notify("textDocument/didClose", Some(serde_json::to_value(params)?))
    }

    /// Get hover information
    pub async fn hover(&mut self, uri: &str, line: u32, character: u32) -> Result<Option<Hover>> {
        let params = HoverParams {
            text_document: TextDocumentIdentifier {
                uri: uri.to_string(),
            },
            position: Position { line, character },
        };

        let result: Option<Hover> = self
            .request("textDocument/hover", Some(serde_json::to_value(params)?))
            .await?;

        Ok(result)
    }

    /// Check if server is running
    pub fn is_running(&self) -> bool {
        self.process.is_some() && self.initialized
    }

    /// Shutdown the server
    pub async fn shutdown(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }

        // Send shutdown request
        let _: serde_json::Value = self.request("shutdown", None).await?;

        // Send exit notification
        self.notify("exit", None)?;

        // Wait for process to exit
        if let Some(mut process) = self.process.take() {
            let _ = process.wait();
        }

        self.initialized = false;
        Ok(())
    }

    /// Get server capabilities
    pub fn capabilities(&self) -> Option<&ServerCapabilities> {
        self.capabilities.as_ref()
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Kill the process if still running
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}
