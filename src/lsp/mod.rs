//! Language Server Protocol (LSP) integration
//!
//! This module provides LSP client functionality to connect to language servers
//! and expose diagnostics, hover information, and other code intelligence
//! features to the AI assistant.
//!
//! Inspired by OpenCode and Crush's LSP implementations.

mod client;
mod config;
mod download;
mod manager;
mod protocol;

pub use client::LspClient;
pub use config::{default_lsp_configs, LspConfig, LspServerConfig};
pub use download::{
    get_effective_binary_path, get_install_info_for_language, get_lsp_install_info,
    install_lsp_server, is_lsp_installed, lsp_servers_dir, InstallMethod, LspInstallInfo,
};
pub use manager::{Diagnostic, DiagnosticSeverity, LspManager, LspStatus};
pub use protocol::{Location, Position, Range};
