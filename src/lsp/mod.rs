//! Language Server Protocol (LSP) integration
//!
//! This module provides LSP client functionality to connect to language servers
//! and expose diagnostics, hover information, and other code intelligence
//! features to the AI assistant.
//!
//! Inspired by OpenCode and Crush's LSP implementations.

mod client;
mod config;
mod manager;
mod protocol;

pub use client::LspClient;
pub use config::{default_lsp_configs, LspConfig, LspServerConfig};
pub use manager::{Diagnostic, DiagnosticSeverity, LspManager, LspStatus};
pub use protocol::{Location, Position, Range};
