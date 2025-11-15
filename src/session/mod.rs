use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::config::Config;
use crate::isolation::{IsolationBackend, create_backend};
use crate::llm::{ContentBlock, LlmClient, Message, ToolDefinition, create_client};
use crate::tools::ToolRegistry;

pub struct Session {
    _config: Config,
    isolation: Box<dyn IsolationBackend>,
    llm_client: Box<dyn LlmClient>,
    tool_registry: ToolRegistry,
    messages: Vec<Message>,
    _project_path: PathBuf,
}

impl Session {
    pub async fn new(config: Config, project_path: PathBuf) -> Result<Self> {
        let isolation = create_backend(&config).await?;
        let llm_client = create_client(&config.llm)?;
        let tool_registry = ToolRegistry::new();

        Ok(Self {
            _config: config,
            isolation,
            llm_client,
            tool_registry,
            messages: vec![],
            _project_path: project_path,
        })
    }

    pub async fn start(&mut self, project_path: PathBuf) -> Result<()> {
        tracing::info!("🔒 Starting isolated environment for project: {:?}", project_path);
        self.isolation.start(project_path).await?;
        tracing::info!("✓ Isolation active: {}", self.isolation.backend_name());
        Ok(())
    }

    pub async fn send_message(&mut self, user_message: String) -> Result<String> {
        // Add user message to history
        self.messages.push(Message::user(user_message));

        let mut response_text = String::new();

        loop {
            // Get tools schema
            let tools: Vec<ToolDefinition> = self.tool_registry
                .get_tools_schema()
                .into_iter()
                .map(|schema| ToolDefinition {
                    name: schema["name"].as_str().unwrap().to_string(),
                    description: schema["description"].as_str().unwrap().to_string(),
                    input_schema: schema["input_schema"].clone(),
                })
                .collect();

            // Send to LLM
            let assistant_message = self.llm_client
                .send_message(&self.messages, &tools)
                .await?;

            // Check if there are any tool calls
            let has_tool_calls = assistant_message.content.iter()
                .any(|c| matches!(c, ContentBlock::ToolUse { .. }));

            // Extract text from response
            for block in &assistant_message.content {
                if let ContentBlock::Text { text } = block {
                    response_text.push_str(text);
                    response_text.push('\n');
                }
            }

            // Add assistant message to history
            self.messages.push(assistant_message.clone());

            if !has_tool_calls {
                // No tool calls, we're done
                break;
            }

            // Execute tool calls
            let mut tool_results = Vec::new();
            let mut tools_executed = Vec::new();

            for block in &assistant_message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    tracing::info!("🔒 Executing tool in {} sandbox: {}", self.isolation.backend_name(), name);

                    // 🔒 SECURITY: Require isolation to be active - no fallback to host
                    let working_dir = self.isolation
                        .get_sandbox_dir()
                        .context("Isolation not active - tool execution requires active sandbox")?;

                    let result = match self.tool_registry.get_tool(name) {
                        Some(tool) => {
                            match tool.execute(input.clone(), working_dir).await {
                                Ok(output) => {
                                    tools_executed.push(name.clone());
                                    output
                                }
                                Err(e) => format!("Error: {}", e),
                            }
                        }
                        None => format!("Error: Unknown tool '{}'", name),
                    };

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: result,
                    });
                }
            }

            // 🔒 Auto-commit changes after tool execution
            if !tools_executed.is_empty() {
                let commit_message = format!("Agent executed: {}", tools_executed.join(", "));
                if let Err(e) = self.isolation.commit_changes(&commit_message).await {
                    tracing::warn!("Failed to auto-commit changes: {}", e);
                } else {
                    tracing::debug!("✓ Auto-committed: {}", commit_message);
                }
            }

            // Add tool results as a new user message
            if !tool_results.is_empty() {
                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: tool_results,
                });
            }
        }

        Ok(response_text.trim().to_string())
    }

    pub async fn stop(&mut self) -> Result<()> {
        tracing::info!("Stopping {} and syncing files", self.isolation.backend_name());
        self.isolation.stop().await?;
        Ok(())
    }
}
