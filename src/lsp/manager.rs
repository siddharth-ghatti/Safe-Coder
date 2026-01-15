//! LSP Manager
//!
//! Manages multiple language server clients and provides a unified interface
//! for diagnostics and code intelligence features.

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::client::LspClient;
use super::config::{
    default_lsp_configs, detect_language_id, extension_to_language_id, LspConfig,
};
use super::download::{
    get_effective_binary_path, get_install_info_for_language, install_lsp_server,
};
use super::protocol::LspDiagnostic;

/// Diagnostic severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    pub fn from_lsp(severity: Option<i32>) -> Self {
        match severity {
            Some(1) => DiagnosticSeverity::Error,
            Some(2) => DiagnosticSeverity::Warning,
            Some(3) => DiagnosticSeverity::Information,
            Some(4) => DiagnosticSeverity::Hint,
            _ => DiagnosticSeverity::Information,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            DiagnosticSeverity::Error => "●",
            DiagnosticSeverity::Warning => "▲",
            DiagnosticSeverity::Information => "ℹ",
            DiagnosticSeverity::Hint => "💡",
        }
    }
}

/// A diagnostic message from an LSP server
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// File path (relative to project root)
    pub file: String,
    /// Line number (1-indexed for display)
    pub line: u32,
    /// Column number (1-indexed for display)
    pub column: u32,
    /// Severity level
    pub severity: DiagnosticSeverity,
    /// Diagnostic message
    pub message: String,
    /// Source (e.g., "rust-analyzer", "gopls")
    pub source: Option<String>,
    /// Error code if available
    pub code: Option<String>,
}

impl Diagnostic {
    /// Format diagnostic for display
    pub fn format(&self) -> String {
        let severity = self.severity.as_str();
        let icon = self.severity.icon();
        let source = self.source.as_deref().unwrap_or("lsp");
        let code_str = self
            .code
            .as_ref()
            .map(|c| format!("[{}]", c))
            .unwrap_or_default();

        format!(
            "{} {}:{}:{} {} {}{}: {}",
            icon, self.file, self.line, self.column, severity, source, code_str, self.message
        )
    }

    /// Format for AI context (compact)
    pub fn format_for_ai(&self) -> String {
        format!(
            "{}:{}:{}: {}: {}",
            self.file,
            self.line,
            self.column,
            self.severity.as_str(),
            self.message
        )
    }
}

/// Status of an LSP server
#[derive(Debug, Clone)]
pub struct LspStatus {
    /// Language name
    pub language: String,
    /// Server command
    pub command: String,
    /// Whether the server is running
    pub running: bool,
    /// Whether the server command is available
    pub available: bool,
    /// Number of open documents
    pub open_documents: usize,
    /// Number of diagnostics
    pub diagnostic_count: usize,
}

/// LSP Manager - manages multiple language server clients
pub struct LspManager {
    /// Project root path
    root_path: PathBuf,
    /// LSP configuration
    config: LspConfig,
    /// Active LSP clients by language ID
    clients: HashMap<String, LspClient>,
    /// Diagnostics by file URI
    diagnostics: Arc<RwLock<HashMap<String, Vec<Diagnostic>>>>,
    /// Document versions by URI
    document_versions: HashMap<String, i32>,
    /// Whether manager is initialized
    initialized: bool,
}

impl LspManager {
    /// Create a new LSP manager
    pub fn new(root_path: PathBuf, config: Option<LspConfig>) -> Self {
        let mut lsp_config = config.unwrap_or_default();

        // Merge with defaults
        let defaults = default_lsp_configs();
        for (lang, default_config) in defaults {
            lsp_config.servers.entry(lang).or_insert(default_config);
        }

        Self {
            root_path,
            config: lsp_config,
            clients: HashMap::new(),
            diagnostics: Arc::new(RwLock::new(HashMap::new())),
            document_versions: HashMap::new(),
            initialized: false,
        }
    }

    /// Initialize LSP servers based on project files
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Detect languages in the project
        let languages = self.detect_project_languages()?;

        // Start servers for detected languages
        for lang in languages {
            if let Err(e) = self.start_server(&lang).await {
                // Log but don't fail - server might not be installed
                tracing::warn!("Failed to start {} LSP: {}", lang, e);
            }
        }

        self.initialized = true;
        Ok(())
    }

    /// Detect languages used in the project
    fn detect_project_languages(&self) -> Result<Vec<String>> {
        let mut languages = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Check for root markers
        for (lang, config) in &self.config.servers {
            if config.disabled {
                continue;
            }

            for marker in &config.root_markers {
                if self.root_path.join(marker).exists() && seen.insert(lang.clone()) {
                    languages.push(lang.clone());
                    break;
                }
            }
        }

        // Also scan for common file extensions in top-level and src/
        let scan_dirs = vec![
            self.root_path.clone(),
            self.root_path.join("src"),
            self.root_path.join("lib"),
        ];

        for dir in scan_dirs {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if let Some(lang) = detect_language_id(&format!(".{}", ext)) {
                                if seen.insert(lang.clone()) {
                                    languages.push(lang);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(languages)
    }

    /// Start a language server
    ///
    /// This method is designed to be resilient - if a server fails to start
    /// (due to download failures, network issues, missing dependencies, etc.),
    /// it will log the error and return Ok(()) to allow the CLI to continue.
    async fn start_server(&mut self, language: &str) -> Result<()> {
        if self.clients.contains_key(language) {
            return Ok(());
        }

        let mut config = match self.config.servers.get(language).cloned() {
            Some(c) => c,
            None => {
                // No config for this language - not an error, just skip
                return Ok(());
            }
        };

        if config.disabled {
            return Ok(());
        }

        // Check if server is available, try auto-download if not
        let binary_path = get_effective_binary_path(&config.command);

        if binary_path.is_none() {
            // Try to auto-download the server with a timeout
            if let Some(install_info) = get_install_info_for_language(language) {
                tracing::info!(
                    "LSP server '{}' not found, attempting to install...",
                    config.command
                );

                // Use a timeout for downloads to prevent blocking the CLI
                let download_timeout = std::time::Duration::from_secs(60);
                match tokio::time::timeout(download_timeout, install_lsp_server(&install_info))
                    .await
                {
                    Ok(Ok(installed_path)) => {
                        tracing::info!(
                            "Successfully installed {} to {}",
                            config.command,
                            installed_path.display()
                        );
                        // Update config to use the installed binary
                        config.command = installed_path.to_string_lossy().to_string();
                    }
                    Ok(Err(e)) => {
                        // Download failed - log and continue without this LSP
                        tracing::warn!(
                            "LSP auto-install failed for {} (continuing without LSP support for {}): {}",
                            config.command, language, e
                        );
                        return Ok(());
                    }
                    Err(_) => {
                        // Download timed out - log and continue without this LSP
                        tracing::warn!(
                            "LSP auto-install timed out for {} (continuing without LSP support for {})",
                            config.command, language
                        );
                        return Ok(());
                    }
                }
            } else {
                // No auto-install available - this is fine, just continue without this LSP
                tracing::debug!(
                    "LSP server '{}' not found (no auto-install available for {})",
                    config.command, language
                );
                return Ok(());
            }
        } else if let Some(path) = binary_path {
            // Use the effective path (could be from PATH or our install dir)
            config.command = path.to_string_lossy().to_string();
        }

        let mut client = LspClient::new(language.to_string(), config.clone());

        if !client.is_available() {
            // Server binary exists but isn't available - log and continue
            tracing::debug!(
                "LSP server '{}' not available for {} (continuing without LSP)",
                config.command, language
            );
            return Ok(());
        }

        // Try to start and initialize the client
        if let Err(e) = client.start(&self.root_path) {
            tracing::warn!(
                "Failed to start LSP server for {} (continuing without LSP): {}",
                language, e
            );
            return Ok(());
        }

        if let Err(e) = client.initialize().await {
            tracing::warn!(
                "Failed to initialize LSP server for {} (continuing without LSP): {}",
                language, e
            );
            return Ok(());
        }

        self.clients.insert(language.to_string(), client);
        Ok(())
    }

    /// Open a document and get diagnostics
    pub async fn open_document(&mut self, path: &Path) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let lang = detect_language_id(&format!(".{}", ext));

        if lang.is_none() {
            return Ok(());
        }
        let lang = lang.unwrap();

        // Start server if not running
        if !self.clients.contains_key(&lang) {
            if let Err(_) = self.start_server(&lang).await {
                return Ok(()); // Server not available, skip
            }
        }

        let client = match self.clients.get_mut(&lang) {
            Some(c) => c,
            None => return Ok(()),
        };

        let uri = format!("file://{}", path.display());
        let content = std::fs::read_to_string(path)?;
        let version = self.document_versions.entry(uri.clone()).or_insert(0);
        *version += 1;

        let lsp_language_id = extension_to_language_id(ext).unwrap_or(&lang);
        client.did_open(&uri, lsp_language_id, *version, &content)?;

        Ok(())
    }

    /// Update a document
    pub async fn update_document(&mut self, path: &Path, content: &str) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let lang = match detect_language_id(&format!(".{}", ext)) {
            Some(l) => l,
            None => return Ok(()),
        };

        let client = match self.clients.get_mut(&lang) {
            Some(c) => c,
            None => return Ok(()),
        };

        let uri = format!("file://{}", path.display());
        let version = self.document_versions.entry(uri.clone()).or_insert(0);
        *version += 1;

        client.did_change(&uri, *version, content)?;

        Ok(())
    }

    /// Notify LSP that a file has changed (convenience method)
    ///
    /// This reads the file content and notifies the LSP server.
    /// If the file wasn't previously opened, it opens it first.
    pub async fn notify_file_changed(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;

        // Check if document is already open
        let uri = format!("file://{}", path.display());
        if self.document_versions.contains_key(&uri) {
            // Update existing document
            self.update_document(path, &content).await?;
        } else {
            // Open the document first
            self.open_document(path).await?;
        }

        Ok(())
    }

    /// Close a document
    pub async fn close_document(&mut self, path: &Path) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let lang = match detect_language_id(&format!(".{}", ext)) {
            Some(l) => l,
            None => return Ok(()),
        };

        let client = match self.clients.get_mut(&lang) {
            Some(c) => c,
            None => return Ok(()),
        };

        let uri = format!("file://{}", path.display());
        client.did_close(&uri)?;
        self.document_versions.remove(&uri);

        Ok(())
    }

    /// Get all diagnostics
    pub async fn get_all_diagnostics(&self) -> Vec<Diagnostic> {
        let diags = self.diagnostics.read().await;
        diags.values().flatten().cloned().collect()
    }

    /// Get diagnostics for a specific file
    pub async fn get_file_diagnostics(&self, path: &Path) -> Vec<Diagnostic> {
        let uri = format!("file://{}", path.display());
        let diags = self.diagnostics.read().await;
        diags.get(&uri).cloned().unwrap_or_default()
    }

    /// Get diagnostic counts (errors, warnings) for all files
    pub async fn get_diagnostic_counts(&self) -> (usize, usize) {
        let all_diags = self.get_all_diagnostics().await;
        let errors = all_diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warnings = all_diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();
        (errors, warnings)
    }

    /// Get diagnostic counts for a specific file
    pub async fn get_file_diagnostic_counts(&self, path: &Path) -> (usize, usize) {
        let diags = self.get_file_diagnostics(path).await;
        let errors = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .count();
        let warnings = diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Warning)
            .count();
        (errors, warnings)
    }

    /// Get diagnostics summary for AI context
    pub async fn get_diagnostics_summary(&self) -> String {
        let all_diags = self.get_all_diagnostics().await;

        if all_diags.is_empty() {
            return String::new();
        }

        // Only report ERRORS, ignore warnings
        // This prevents the agent from looping on warning fixes
        let errors: Vec<_> = all_diags
            .iter()
            .filter(|d| d.severity == DiagnosticSeverity::Error)
            .collect();

        // If no errors (only warnings), return empty - we ignore warnings
        if errors.is_empty() {
            return String::new();
        }

        let mut summary = String::from("## LSP Errors\n\n");

        summary.push_str(&format!("### Errors ({})\n", errors.len()));
        for diag in errors.iter().take(10) {
            summary.push_str(&format!("- {}\n", diag.format_for_ai()));
        }
        if errors.len() > 10 {
            summary.push_str(&format!("... and {} more errors\n", errors.len() - 10));
        }

        summary
    }

    /// Get status of all LSP servers
    pub fn get_status(&self) -> Vec<LspStatus> {
        let mut statuses = Vec::new();

        for (lang, config) in &self.config.servers {
            if config.disabled {
                continue;
            }

            let client = self.clients.get(lang);
            let running = client.map(|c| c.is_running()).unwrap_or(false);

            // Check availability - handle both absolute paths and command names
            let available = {
                let path = std::path::Path::new(&config.command);
                if path.is_absolute() {
                    path.exists() && path.is_file()
                } else {
                    which::which(&config.command).is_ok()
                }
            };

            statuses.push(LspStatus {
                language: lang.clone(),
                command: config.command.clone(),
                running,
                available,
                open_documents: 0,   // TODO: track this
                diagnostic_count: 0, // TODO: track this
            });
        }

        // Sort by running status, then by language name
        statuses.sort_by(|a, b| {
            b.running
                .cmp(&a.running)
                .then_with(|| a.language.cmp(&b.language))
        });

        statuses
    }

    /// Get running servers
    pub fn get_running_servers(&self) -> Vec<&str> {
        self.clients
            .iter()
            .filter(|(_, c)| c.is_running())
            .map(|(lang, _)| lang.as_str())
            .collect()
    }

    /// Get all clients (for status display)
    pub fn get_clients(&self) -> impl Iterator<Item = (&String, &LspClient)> {
        self.clients.iter()
    }

    /// Shutdown all servers
    pub async fn shutdown(&mut self) -> Result<()> {
        for (_, client) in self.clients.iter_mut() {
            let _ = client.shutdown().await;
        }
        self.clients.clear();
        self.initialized = false;
        Ok(())
    }

    /// Check if manager is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Add diagnostics received from a server
    pub async fn add_diagnostics(&self, uri: String, lsp_diagnostics: Vec<LspDiagnostic>) {
        let file_path = uri.strip_prefix("file://").unwrap_or(&uri);
        let relative_path = Path::new(file_path)
            .strip_prefix(&self.root_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string());

        let diagnostics: Vec<Diagnostic> = lsp_diagnostics
            .into_iter()
            .map(|d| Diagnostic {
                file: relative_path.clone(),
                line: d.range.start.line + 1,
                column: d.range.start.character + 1,
                severity: DiagnosticSeverity::from_lsp(d.severity),
                message: d.message,
                source: d.source,
                code: d.code.map(|c| match c {
                    serde_json::Value::String(s) => s,
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                }),
            })
            .collect();

        let mut diags = self.diagnostics.write().await;
        diags.insert(uri, diagnostics);
    }
}

impl Drop for LspManager {
    fn drop(&mut self) {
        // Clients will be dropped and killed automatically
    }
}
