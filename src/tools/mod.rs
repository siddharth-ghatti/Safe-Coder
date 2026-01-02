use anyhow::Result;
use async_trait::async_trait;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::ToolConfig;

/// Agent execution mode - controls which tools are available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMode {
    /// Plan mode: Read-only exploration tools only
    /// Use this to explore, understand, and plan before making changes
    Plan,
    /// Build mode: Full tool access including file modifications and bash
    #[default]
    Build,
}

impl AgentMode {
    /// Get the list of tool names available in this mode
    pub fn enabled_tools(&self) -> &'static [&'static str] {
        match self {
            AgentMode::Plan => &[
                "read_file", // Read files
                "list_file", // List directories
                "glob",      // Find files by pattern
                "grep",      // Search file contents
                "webfetch",  // Fetch web content
                "todoread",  // Read task list
            ],
            AgentMode::Build => &[
                "read_file",
                "write_file",
                "edit_file",
                "list_file",
                "glob",
                "grep",
                "bash",
                "webfetch",
                "todowrite",
                "todoread",
                // "subagent", // Disabled for now - perfecting planning first
            ],
        }
    }

    /// Check if a specific tool is enabled in this mode
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        self.enabled_tools().contains(&tool_name)
    }

    /// Get a description of this mode for display
    pub fn description(&self) -> &'static str {
        match self {
            AgentMode::Plan => {
                "Read-only exploration mode. Analyze the codebase before making changes."
            }
            AgentMode::Build => "Full execution mode. Can modify files and run commands.",
        }
    }

    /// Get short display name
    pub fn short_name(&self) -> &'static str {
        match self {
            AgentMode::Plan => "PLAN",
            AgentMode::Build => "BUILD",
        }
    }

    /// Cycle to next mode
    pub fn next(self) -> Self {
        match self {
            AgentMode::Plan => AgentMode::Build,
            AgentMode::Build => AgentMode::Plan,
        }
    }
}

impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod list;
pub mod read;
pub mod subagent;
pub mod todo;
pub mod webfetch;
pub mod write;

pub use bash::BashTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use list::ListTool;
pub use read::ReadTool;
pub use subagent::SubagentTool;
pub use todo::{TodoReadTool, TodoWriteTool};
pub use webfetch::WebFetchTool;
pub use write::WriteTool;

/// Callback type for streaming output updates
pub type OutputCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Context passed to tool execution containing working directory and configuration
#[derive(Clone)]
pub struct ToolContext<'a> {
    pub working_dir: &'a Path,
    pub config: &'a ToolConfig,
    /// Optional callback for streaming output (used by bash tool)
    pub output_callback: Option<OutputCallback>,
    /// Optional session event sender for subagent streaming
    pub session_event_tx: Option<mpsc::UnboundedSender<crate::session::SessionEvent>>,
}

impl<'a> ToolContext<'a> {
    pub fn new(working_dir: &'a Path, config: &'a ToolConfig) -> Self {
        Self {
            working_dir,
            config,
            output_callback: None,
            session_event_tx: None,
        }
    }

    pub fn with_output_callback(
        working_dir: &'a Path,
        config: &'a ToolConfig,
        callback: OutputCallback,
    ) -> Self {
        Self {
            working_dir,
            config,
            output_callback: Some(callback),
            session_event_tx: None,
        }
    }

    pub fn with_session_events(
        mut self,
        tx: mpsc::UnboundedSender<crate::session::SessionEvent>,
    ) -> Self {
        self.session_event_tx = Some(tx);
        self
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
    subagent_tool: Option<Arc<SubagentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: vec![],
            subagent_tool: None,
        }
    }

    /// Create a registry without subagent support (for use in subagents themselves)
    pub fn new_without_subagents() -> Self {
        let mut registry = Self {
            tools: vec![],
            subagent_tool: None,
        };
        // File operations
        registry.register(Box::new(ReadTool));
        registry.register(Box::new(WriteTool));
        registry.register(Box::new(EditTool));
        registry.register(Box::new(ListTool));
        // Search tools
        registry.register(Box::new(GlobTool));
        registry.register(Box::new(GrepTool));
        // Shell execution
        registry.register(Box::new(BashTool));
        // Web access
        registry.register(Box::new(WebFetchTool));
        // Task tracking
        registry.register(Box::new(TodoWriteTool));
        registry.register(Box::new(TodoReadTool));
        registry
    }

    /// Initialize the registry with subagent support
    /// Optionally accepts a session event sender for live streaming of subagent output
    pub async fn with_subagent_support(
        mut self,
        config: crate::config::Config,
        project_path: std::path::PathBuf,
    ) -> Self {
        self.init_subagent_support(config, project_path, None).await
    }

    /// Initialize the registry with subagent support and session event forwarding
    pub async fn with_subagent_support_and_events(
        mut self,
        config: crate::config::Config,
        project_path: std::path::PathBuf,
        session_event_tx: mpsc::UnboundedSender<crate::session::SessionEvent>,
    ) -> Self {
        self.init_subagent_support(config, project_path, Some(session_event_tx))
            .await
    }

    async fn init_subagent_support(
        mut self,
        config: crate::config::Config,
        project_path: std::path::PathBuf,
        session_event_tx: Option<mpsc::UnboundedSender<crate::session::SessionEvent>>,
    ) -> Self {
        use crate::session::SessionEvent;
        use crate::subagent::SubagentEvent;

        // Register all basic tools first
        // File operations
        self.register(Box::new(ReadTool));
        self.register(Box::new(WriteTool));
        self.register(Box::new(EditTool));
        self.register(Box::new(ListTool));
        // Search tools
        self.register(Box::new(GlobTool));
        self.register(Box::new(GrepTool));
        // Shell execution
        self.register(Box::new(BashTool));
        // Web access
        self.register(Box::new(WebFetchTool));
        // Task tracking
        self.register(Box::new(TodoWriteTool));
        self.register(Box::new(TodoReadTool));

        // Create event channel for subagent communication
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SubagentEvent>();

        // Spawn background task to forward subagent events to session
        let forward_tx: Option<mpsc::UnboundedSender<SessionEvent>> = session_event_tx;
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Convert SubagentEvent to SessionEvent and forward
                if let Some(ref tx) = forward_tx {
                    let session_event: SessionEvent = match event {
                        SubagentEvent::Started { id, kind, task } => {
                            SessionEvent::SubagentStarted {
                                id,
                                kind: kind.display_name().to_string(),
                                task,
                            }
                        }
                        SubagentEvent::Thinking { id, message } => {
                            SessionEvent::SubagentProgress { id, message }
                        }
                        SubagentEvent::ToolStart {
                            id,
                            tool_name,
                            description,
                        } => SessionEvent::SubagentToolUsed {
                            id,
                            tool: tool_name,
                            description,
                        },
                        SubagentEvent::ToolOutput {
                            id,
                            tool_name,
                            output,
                        } => SessionEvent::SubagentProgress {
                            id,
                            message: format!(
                                "{}: {}",
                                tool_name,
                                if output.len() > 200 {
                                    format!("{}...", &output[..200])
                                } else {
                                    output
                                }
                            ),
                        },
                        SubagentEvent::ToolComplete {
                            id,
                            tool_name,
                            success,
                        } => SessionEvent::SubagentProgress {
                            id,
                            message: format!("{} {}", tool_name, if success { "✓" } else { "✗" }),
                        },
                        SubagentEvent::TextChunk { id, text } => SessionEvent::SubagentProgress {
                            id,
                            message: if text.len() > 300 {
                                format!("{}...", &text[..300])
                            } else {
                                text
                            },
                        },
                        SubagentEvent::IterationComplete {
                            id,
                            iteration,
                            max_iterations,
                        } => SessionEvent::SubagentProgress {
                            id,
                            message: format!("Iteration {}/{}", iteration, max_iterations),
                        },
                        SubagentEvent::Completed {
                            id,
                            success,
                            summary,
                        } => SessionEvent::SubagentCompleted {
                            id,
                            success,
                            summary,
                        },
                        SubagentEvent::Error { id, error } => SessionEvent::SubagentProgress {
                            id,
                            message: format!("Error: {}", error),
                        },
                    };
                    let _ = tx.send(session_event);
                }
            }
        });

        // Create and initialize subagent tool
        let subagent_tool = Arc::new(SubagentTool::new());
        subagent_tool
            .initialize(config, project_path, event_tx)
            .await;

        // Store reference and register
        self.subagent_tool = Some(subagent_tool.clone());
        self.register(Box::new(SubagentToolWrapper {
            inner: subagent_tool,
        }));

        self
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn get_tools_schema(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.parameters_schema()
                })
            })
            .collect()
    }

    /// Get tool schemas filtered by agent mode
    /// Only returns tools that are enabled for the given mode
    pub fn get_tools_schema_for_mode(&self, mode: AgentMode) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .filter(|tool| mode.is_tool_enabled(tool.name()))
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.parameters_schema()
                })
            })
            .collect()
    }

    /// Check if a tool can be executed in the given mode
    pub fn can_execute_in_mode(&self, tool_name: &str, mode: AgentMode) -> bool {
        mode.is_tool_enabled(tool_name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper to make Arc<SubagentTool> usable as Box<dyn Tool>
struct SubagentToolWrapper {
    inner: Arc<SubagentTool>,
}

#[async_trait]
impl Tool for SubagentToolWrapper {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn parameters_schema(&self) -> serde_json::Value {
        self.inner.parameters_schema()
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        self.inner.execute(params, ctx).await
    }
}
