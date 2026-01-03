//! MCP client - manages connection to a single MCP server using rmcp SDK.

use anyhow::{Context, Result};
use rmcp::{
    model::{CallToolRequestParam, Tool as RmcpTool},
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::config::McpServerConfig;

/// State of an MCP client connection.
#[derive(Debug, Clone, PartialEq)]
pub enum McpClientState {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

/// Tool definition from MCP server.
#[derive(Debug, Clone)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

impl From<RmcpTool> for McpToolDefinition {
    fn from(tool: RmcpTool) -> Self {
        Self {
            name: tool.name.to_string(),
            description: tool.description.map(|d| d.to_string()),
            input_schema: serde_json::to_value(&tool.input_schema).unwrap_or_default(),
        }
    }
}

/// The rmcp client handle type.
type RmcpClientHandle = rmcp::service::RunningService<rmcp::RoleClient, ()>;

/// Client for a single MCP server using rmcp SDK.
pub struct McpClient {
    config: McpServerConfig,
    service: Option<RmcpClientHandle>,
    state: RwLock<McpClientState>,
    tools: RwLock<Vec<McpToolDefinition>>,
}

impl McpClient {
    /// Create a new MCP client with the given configuration.
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            service: None,
            state: RwLock::new(McpClientState::Disconnected),
            tools: RwLock::new(Vec::new()),
        }
    }

    /// Connect to the MCP server and initialize the connection.
    pub async fn connect(&mut self, working_dir: &Path) -> Result<()> {
        *self.state.write().await = McpClientState::Connecting;

        // Determine working directory
        let work_dir = self
            .config
            .working_dir
            .as_ref()
            .map(|p| Path::new(p).to_path_buf())
            .unwrap_or_else(|| working_dir.to_path_buf());

        // Clone config values for the closure
        let args = self.config.args.clone();
        let env = self.config.env.clone();

        // Create transport and connect using rmcp pattern
        let transport =
            TokioChildProcess::new(Command::new(&self.config.command).configure(move |cmd| {
                cmd.args(&args);
                cmd.current_dir(&work_dir);
                for (k, v) in &env {
                    cmd.env(k, v);
                }
            }))
            .map_err(|e| {
                anyhow::anyhow!("Failed to spawn MCP server '{}': {}", self.config.name, e)
            })?;

        let service = ().serve(transport).await.map_err(|e| {
            let err_msg = e.to_string();
            anyhow::anyhow!(
                "Failed to initialize MCP server '{}': {}",
                self.config.name,
                err_msg
            )
        })?;

        self.service = Some(service);

        // Discover tools
        if let Err(e) = self.refresh_tools().await {
            *self.state.write().await = McpClientState::Failed(e.to_string());
            return Err(e);
        }

        *self.state.write().await = McpClientState::Connected;
        Ok(())
    }

    /// Refresh the list of available tools from the server.
    pub async fn refresh_tools(&self) -> Result<()> {
        let service = self.service.as_ref().context("Not connected")?;

        let tools_list = service
            .list_all_tools()
            .await
            .context("Failed to list tools")?;

        let tools: Vec<McpToolDefinition> = tools_list
            .into_iter()
            .map(McpToolDefinition::from)
            .collect();

        *self.tools.write().await = tools;
        Ok(())
    }

    /// Call a tool on this server.
    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<String> {
        let service = self.service.as_ref().context("Not connected")?;

        let args =
            if arguments.is_null() || arguments.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                None
            } else {
                arguments.as_object().cloned()
            };

        let tool_name = name.to_string();
        let result = service
            .call_tool(CallToolRequestParam {
                name: tool_name.clone().into(),
                arguments: args,
            })
            .await
            .with_context(|| format!("Failed to call tool '{}'", tool_name))?;

        // Extract text content from result
        let mut output = String::new();
        for content in result.content {
            if let Some(text) = content.as_text() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&text.text);
            }
        }

        if result.is_error.unwrap_or(false) {
            anyhow::bail!("Tool '{}' returned error: {}", name, output);
        }

        Ok(output)
    }

    /// Get the list of discovered tools.
    pub async fn get_tools(&self) -> Vec<McpToolDefinition> {
        self.tools.read().await.clone()
    }

    /// Get the server name from configuration.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Get the agent mode restriction for this server.
    pub fn mode(&self) -> &str {
        &self.config.mode
    }

    /// Get the current connection state.
    pub async fn state(&self) -> McpClientState {
        self.state.read().await.clone()
    }

    /// Check if the server is connected.
    pub async fn is_connected(&self) -> bool {
        *self.state.read().await == McpClientState::Connected && self.service.is_some()
    }

    /// Disconnect from the server.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(service) = self.service.take() {
            // Cancel the service to properly close the connection
            let _ = service.cancel().await;
        }
        *self.state.write().await = McpClientState::Disconnected;
        self.tools.write().await.clear();
        Ok(())
    }
}

/// A connected MCP client wrapped in Arc for sharing.
pub type SharedMcpClient = Arc<RwLock<McpClient>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = McpServerConfig {
            name: "test".to_string(),
            command: "echo".to_string(),
            ..Default::default()
        };
        let client = McpClient::new(config);
        assert_eq!(client.name(), "test");
        assert_eq!(client.mode(), "both");
    }
}
