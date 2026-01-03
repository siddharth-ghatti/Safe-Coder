//! MCP configuration types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level MCP configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct McpConfig {
    /// Whether MCP is enabled globally.
    #[serde(default)]
    pub enabled: bool,
    /// List of MCP server configurations.
    #[serde(default)]
    pub servers: Vec<McpServerConfig>,
}

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerConfig {
    /// Unique name for this server (used for tool namespacing).
    pub name: String,
    /// Command to spawn the server process.
    pub command: String,
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory (optional, defaults to project directory).
    #[serde(default)]
    pub working_dir: Option<String>,
    /// Timeout for initialization in seconds (default: 30).
    #[serde(default = "default_init_timeout")]
    pub init_timeout_secs: u64,
    /// Timeout for tool calls in seconds (default: 120).
    #[serde(default = "default_call_timeout")]
    pub call_timeout_secs: u64,
    /// Agent mode restriction: "plan", "build", or "both" (default: "both").
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Whether this server is disabled.
    #[serde(default)]
    pub disabled: bool,
}

fn default_init_timeout() -> u64 {
    30
}

fn default_call_timeout() -> u64 {
    120
}

fn default_mode() -> String {
    "both".to_string()
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            init_timeout_secs: default_init_timeout(),
            call_timeout_secs: default_call_timeout(),
            mode: default_mode(),
            disabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = McpConfig::default();
        assert!(!config.enabled);
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_server_config_defaults() {
        let config = McpServerConfig::default();
        assert_eq!(config.init_timeout_secs, 30);
        assert_eq!(config.call_timeout_secs, 120);
        assert_eq!(config.mode, "both");
        assert!(!config.disabled);
    }

    #[test]
    fn test_config_deserialization() {
        let toml = r#"
enabled = true

[[servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@anthropic/mcp-server-filesystem", "/tmp"]
mode = "build"

[[servers]]
name = "weather"
command = "uvx"
args = ["mcp-server-weather"]
"#;
        let config: McpConfig = toml::from_str(toml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.servers.len(), 2);
        assert_eq!(config.servers[0].name, "filesystem");
        assert_eq!(config.servers[0].mode, "build");
        assert_eq!(config.servers[1].name, "weather");
        assert_eq!(config.servers[1].mode, "both"); // default
    }
}
