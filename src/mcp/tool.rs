//! MCP Tool wrapper - implements the Tool trait for MCP-discovered tools.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::client::{McpClient, McpToolDefinition};
use crate::tools::{AgentMode, Tool, ToolContext};

/// Wraps an MCP tool to implement the Tool trait.
pub struct McpTool {
    /// Server name (for namespacing).
    server_name: String,
    /// The namespaced tool name (cached for lifetime).
    namespaced_name: String,
    /// Tool definition from MCP server.
    definition: McpToolDefinition,
    /// Reference to the MCP client.
    client: Arc<RwLock<McpClient>>,
    /// Agent mode restriction ("plan", "build", or "both").
    mode: String,
}

impl McpTool {
    /// Create a new MCP tool wrapper.
    pub fn new(
        server_name: String,
        definition: McpToolDefinition,
        client: Arc<RwLock<McpClient>>,
        mode: String,
    ) -> Self {
        let namespaced_name = format!("mcp_{}_{}", server_name, definition.name);
        Self {
            server_name,
            namespaced_name,
            definition,
            client,
            mode,
        }
    }

    /// Get the original (non-namespaced) tool name.
    pub fn original_name(&self) -> &str {
        &self.definition.name
    }

    /// Get the server name this tool belongs to.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Check if tool is available in the given agent mode.
    pub fn is_available_in_mode(&self, mode: &AgentMode) -> bool {
        match self.mode.as_str() {
            "plan" => matches!(mode, AgentMode::Plan),
            "build" => matches!(mode, AgentMode::Build),
            "both" => true,
            _ => true, // Default to available
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.namespaced_name
    }

    fn description(&self) -> &str {
        self.definition
            .description
            .as_deref()
            .unwrap_or("MCP tool (no description provided)")
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.definition.input_schema.clone()
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext<'_>) -> Result<String> {
        let client = self.client.read().await;
        client.call_tool(&self.definition.name, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::config::McpServerConfig;

    fn create_test_tool() -> McpTool {
        let definition = McpToolDefinition {
            name: "read_file".to_string(),
            description: Some("Read a file from disk".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"]
            }),
        };

        let config = McpServerConfig {
            name: "filesystem".to_string(),
            command: "echo".to_string(),
            mode: "both".to_string(),
            ..Default::default()
        };

        let client = Arc::new(RwLock::new(McpClient::new(config)));

        McpTool::new(
            "filesystem".to_string(),
            definition,
            client,
            "both".to_string(),
        )
    }

    #[test]
    fn test_namespaced_name() {
        let tool = create_test_tool();
        assert_eq!(tool.name(), "mcp_filesystem_read_file");
        assert_eq!(tool.original_name(), "read_file");
        assert_eq!(tool.server_name(), "filesystem");
    }

    #[test]
    fn test_description() {
        let tool = create_test_tool();
        assert_eq!(tool.description(), "Read a file from disk");
    }

    #[test]
    fn test_mode_availability() {
        let tool = create_test_tool();
        assert!(tool.is_available_in_mode(&AgentMode::Plan));
        assert!(tool.is_available_in_mode(&AgentMode::Build));
    }

    #[test]
    fn test_plan_only_mode() {
        let definition = McpToolDefinition {
            name: "query".to_string(),
            description: None,
            input_schema: serde_json::json!({}),
        };

        let config = McpServerConfig::default();
        let client = Arc::new(RwLock::new(McpClient::new(config)));

        let tool = McpTool::new("db".to_string(), definition, client, "plan".to_string());

        assert!(tool.is_available_in_mode(&AgentMode::Plan));
        assert!(!tool.is_available_in_mode(&AgentMode::Build));
    }
}
