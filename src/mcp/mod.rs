//! MCP (Model Context Protocol) client implementation.
//!
//! This module enables safe-coder to act as an MCP client, connecting to
//! user-specified MCP servers and exposing their tools alongside built-in tools.

pub mod client;
pub mod config;
pub mod tool;

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub use client::{McpClient, McpClientState, SharedMcpClient};
pub use config::{McpConfig, McpServerConfig};
pub use tool::McpTool;

use crate::tools::{AgentMode, Tool};

/// Manages all MCP server connections.
pub struct McpManager {
    config: McpConfig,
    clients: HashMap<String, SharedMcpClient>,
    tools: Vec<Arc<McpTool>>,
}

impl McpManager {
    /// Create a new MCP manager with the given configuration.
    pub fn new(config: McpConfig) -> Self {
        Self {
            config,
            clients: HashMap::new(),
            tools: Vec::new(),
        }
    }

    /// Initialize all configured MCP servers.
    ///
    /// Servers that fail to connect are logged but don't block other servers.
    pub async fn initialize(&mut self, working_dir: &Path) -> Result<()> {
        if !self.config.enabled {
            info!("MCP is disabled");
            return Ok(());
        }

        if self.config.servers.is_empty() {
            info!("No MCP servers configured");
            return Ok(());
        }

        info!("Initializing {} MCP server(s)", self.config.servers.len());

        for server_config in &self.config.servers {
            if server_config.disabled {
                info!("MCP server '{}' is disabled, skipping", server_config.name);
                continue;
            }

            info!(
                "Connecting to MCP server '{}': {} {:?}",
                server_config.name, server_config.command, server_config.args
            );

            let mut client = McpClient::new(server_config.clone());

            match client.connect(working_dir).await {
                Ok(()) => {
                    let tools = client.get_tools().await;
                    info!(
                        "Connected to MCP server '{}', discovered {} tool(s)",
                        server_config.name,
                        tools.len()
                    );

                    // Log discovered tools
                    for tool in &tools {
                        info!(
                            "  - mcp_{}_{}: {}",
                            server_config.name,
                            tool.name,
                            tool.description.as_deref().unwrap_or("(no description)")
                        );
                    }

                    let client = Arc::new(RwLock::new(client));
                    self.clients
                        .insert(server_config.name.clone(), client.clone());

                    // Create tool wrappers
                    for tool_def in tools {
                        let mcp_tool = McpTool::new(
                            server_config.name.clone(),
                            tool_def,
                            client.clone(),
                            server_config.mode.clone(),
                        );
                        self.tools.push(Arc::new(mcp_tool));
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to connect to MCP server '{}': {}",
                        server_config.name, e
                    );
                    // Continue with other servers - don't fail the whole initialization
                }
            }
        }

        info!(
            "MCP initialization complete: {} server(s) connected, {} tool(s) available",
            self.clients.len(),
            self.tools.len()
        );

        Ok(())
    }

    /// Get all MCP tools as boxed Tool trait objects.
    pub fn get_tools(&self) -> Vec<Box<dyn Tool>> {
        self.tools
            .iter()
            .map(|t| Box::new(McpToolRef(t.clone())) as Box<dyn Tool>)
            .collect()
    }

    /// Get tools filtered by agent mode.
    pub fn get_tools_for_mode(&self, mode: &AgentMode) -> Vec<Box<dyn Tool>> {
        self.tools
            .iter()
            .filter(|t| t.is_available_in_mode(mode))
            .map(|t| Box::new(McpToolRef(t.clone())) as Box<dyn Tool>)
            .collect()
    }

    /// Get the number of connected servers.
    pub fn connected_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the total number of available tools.
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Check if MCP is enabled and has any connected servers.
    pub fn is_active(&self) -> bool {
        self.config.enabled && !self.clients.is_empty()
    }

    /// Refresh tools from all connected servers.
    pub async fn refresh_all(&self) -> Result<()> {
        for (name, client) in &self.clients {
            let client = client.read().await;
            if let Err(e) = client.refresh_tools().await {
                warn!("Failed to refresh tools from '{}': {}", name, e);
            }
        }
        Ok(())
    }

    /// Shutdown all MCP servers.
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("Shutting down {} MCP server(s)", self.clients.len());

        for (name, client) in self.clients.drain() {
            let mut client = client.write().await;
            if let Err(e) = client.disconnect().await {
                warn!("Error disconnecting from '{}': {}", name, e);
            }
        }

        self.tools.clear();
        Ok(())
    }

    /// Get connection status for all configured servers.
    pub async fn get_status(&self) -> HashMap<String, McpClientState> {
        let mut status = HashMap::new();

        // Include connected servers
        for (name, client) in &self.clients {
            let client = client.read().await;
            status.insert(name.clone(), client.state().await);
        }

        // Include configured but not connected servers
        for server_config in &self.config.servers {
            if !status.contains_key(&server_config.name) {
                if server_config.disabled {
                    status.insert(
                        server_config.name.clone(),
                        McpClientState::Failed("Disabled".to_string()),
                    );
                } else {
                    status.insert(server_config.name.clone(), McpClientState::Disconnected);
                }
            }
        }

        status
    }

    /// Get a list of all tool names.
    pub fn list_tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }
}

/// A reference wrapper for McpTool that implements Tool.
///
/// This allows us to return Box<dyn Tool> while the actual McpTool
/// lives in an Arc for sharing with the client.
struct McpToolRef(Arc<McpTool>);

#[async_trait::async_trait]
impl Tool for McpToolRef {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn description(&self) -> &str {
        self.0.description()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.0.parameters_schema()
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        ctx: &crate::tools::ToolContext<'_>,
    ) -> Result<String> {
        self.0.execute(params, ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_creation() {
        let config = McpConfig::default();
        let manager = McpManager::new(config);
        assert!(!manager.is_active());
        assert_eq!(manager.tool_count(), 0);
    }

    #[tokio::test]
    async fn test_manager_disabled() {
        let config = McpConfig {
            enabled: false,
            servers: vec![],
        };
        let mut manager = McpManager::new(config);
        manager.initialize(Path::new(".")).await.unwrap();
        assert!(!manager.is_active());
    }
}
