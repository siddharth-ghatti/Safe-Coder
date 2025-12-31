//! Subagent Tool
//!
//! Tool implementation that allows the AI to spawn subagents for focused tasks.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::config::Config;
use crate::tools::{Tool, ToolContext};

use super::executor::SubagentExecutor;
use super::types::{SubagentEvent, SubagentKind, SubagentResult, SubagentScope};

/// Tool for spawning subagents
pub struct SubagentTool {
    /// Configuration
    config: Arc<Mutex<Option<Config>>>,
    /// Project path
    project_path: Arc<Mutex<Option<PathBuf>>>,
    /// Event forwarder - sends subagent events to the parent session
    event_forwarder: Arc<Mutex<Option<mpsc::UnboundedSender<SubagentEvent>>>>,
}

impl SubagentTool {
    /// Create a new subagent tool
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(None)),
            project_path: Arc::new(Mutex::new(None)),
            event_forwarder: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the tool with config and project path
    pub async fn initialize(
        &self,
        config: Config,
        project_path: PathBuf,
        event_tx: mpsc::UnboundedSender<SubagentEvent>,
    ) {
        *self.config.lock().await = Some(config);
        *self.project_path.lock().await = Some(project_path);
        *self.event_forwarder.lock().await = Some(event_tx);
    }
}

impl Default for SubagentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct SubagentParams {
    kind: String,
    task: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    file_patterns: Option<Vec<String>>,
}

#[async_trait]
impl Tool for SubagentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "Spawn a specialized subagent to handle a focused task autonomously. Use this for complex subtasks that benefit from dedicated attention. Available kinds: code_analyzer (analyzes code, read-only), tester (creates/runs tests), refactorer (improves code structure), documenter (writes documentation), custom (user-defined role)."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "kind": {
                    "type": "string",
                    "enum": ["code_analyzer", "tester", "refactorer", "documenter", "custom"],
                    "description": "Type of subagent to spawn. code_analyzer: read-only analysis, tester: creates/runs tests, refactorer: improves code, documenter: writes docs, custom: user-defined"
                },
                "task": {
                    "type": "string",
                    "description": "The specific task for the subagent to accomplish. Be clear and specific."
                },
                "role": {
                    "type": "string",
                    "description": "For 'custom' kind only: describe the role and capabilities of this custom agent"
                },
                "file_patterns": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional file patterns to focus on (e.g., ['src/**/*.rs', 'tests/**/*.rs'])"
                }
            },
            "required": ["kind", "task"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        // Parse parameters
        let params: SubagentParams = serde_json::from_value(params)?;

        // Parse kind
        let kind = match params.kind.as_str() {
            "code_analyzer" => SubagentKind::CodeAnalyzer,
            "tester" => SubagentKind::Tester,
            "refactorer" => SubagentKind::Refactorer,
            "documenter" => SubagentKind::Documenter,
            "custom" => SubagentKind::Custom,
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid subagent kind: {}. Valid kinds: code_analyzer, tester, refactorer, documenter, custom",
                    params.kind
                ));
            }
        };

        // Build scope
        let mut scope = SubagentScope::new(&params.task);
        if let Some(role) = params.role {
            scope = scope.with_role(role);
        }
        if let Some(patterns) = params.file_patterns {
            scope = scope.with_file_patterns(patterns);
        }

        // Get config and project path
        let config = self.config.lock().await;
        let project_path = self.project_path.lock().await;
        let event_forwarder = self.event_forwarder.lock().await;

        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Subagent tool not initialized - missing config"))?;

        let project_path = project_path.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Subagent tool not initialized - missing project path")
        })?;

        // Create event channel
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SubagentEvent>();

        // Forward events to parent if available
        let forwarder_tx = event_forwarder.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Just drop events - the subagent tool doesn't have a parent forwarder in this simple implementation
                // Events are captured in the result instead
                drop(event);
            }
        });

        // Create and run executor
        let mut executor =
            SubagentExecutor::new(kind.clone(), scope, project_path.clone(), config, event_tx)
                .await?;

        let subagent_id = executor.id().to_string();

        // Execute with timeout
        let timeout = std::time::Duration::from_secs(300); // 5 minutes
        let result = tokio::time::timeout(timeout, executor.execute()).await;

        match result {
            Ok(Ok(result)) => Ok(format_result(&kind, &subagent_id, &result)),
            Ok(Err(e)) => Ok(format!(
                "{} Subagent {} failed: {}",
                kind.icon(),
                subagent_id,
                e
            )),
            Err(_) => Ok(format!(
                "{} Subagent {} timed out after {} seconds",
                kind.icon(),
                subagent_id,
                timeout.as_secs()
            )),
        }
    }
}

/// Format subagent result for display
fn format_result(kind: &SubagentKind, id: &str, result: &SubagentResult) -> String {
    let status = if result.success {
        "completed"
    } else {
        "failed"
    };
    let mut output = format!(
        "{} Subagent {} ({}) {} in {} iteration(s)\n\n",
        kind.icon(),
        id,
        kind.display_name(),
        status,
        result.iterations
    );

    if !result.output.is_empty() {
        // Truncate very long outputs
        let display_output = if result.output.len() > 2000 {
            format!("{}...\n[truncated]", &result.output[..2000])
        } else {
            result.output.clone()
        };
        output.push_str(&format!("## Output\n{}\n\n", display_output));
    }

    if !result.files_read.is_empty() {
        output.push_str(&format!(
            "## Files Read\n{}\n\n",
            result.files_read.join(", ")
        ));
    }

    if !result.files_modified.is_empty() {
        output.push_str(&format!(
            "## Files Modified\n{}\n\n",
            result.files_modified.join(", ")
        ));
    }

    if !result.errors.is_empty() {
        output.push_str(&format!("## Errors\n{}\n", result.errors.join("\n")));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schema() {
        let tool = SubagentTool::new();
        let schema = tool.parameters_schema();

        assert!(schema["properties"]["kind"].is_object());
        assert!(schema["properties"]["task"].is_object());
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("kind")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("task")));
    }
}
