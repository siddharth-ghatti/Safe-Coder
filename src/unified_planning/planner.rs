//! Unified Planner
//!
//! Creates mode-aware plans using LLM. The planner knows about the execution mode
//! upfront and generates plans optimized for that mode's capabilities.

use anyhow::{Context, Result};
use serde::Deserialize;
use uuid::Uuid;

use crate::llm::{ContentBlock, LlmClient, Message};

use super::types::{
    ExecutionMode, PlanStatus, StepExecutor, StepGroup, SubagentKind, UnifiedPlan, UnifiedStep,
    WorkerKind,
};

/// Mode-specific context for Direct execution
const DIRECT_MODE_CONTEXT: &str = r#"
**Direct Mode**: All steps execute inline in the current Safe-Coder session.
- No parallelism available - steps execute one at a time
- All steps run in the same process and share context
- Best for: Simple tasks, quick fixes, single-file changes
- Limitations: Cannot parallelize work

Plan with sequential groups, each containing a single step.
"#;

/// Mode-specific context for Subagent execution
const SUBAGENT_MODE_CONTEXT: &str = r#"
**Subagent Mode**: Steps can be delegated to specialized internal agents.
- Parallel execution within same process
- Subagent types available:
  - analyzer: Read-only code analysis (glob, grep, read_file)
  - tester: Create and run tests (read, write, edit, bash)
  - refactorer: Improve code structure (read, edit, bash)
  - documenter: Generate documentation (read, write, edit)
- Best for: Medium complexity tasks with focused subtasks
- Agents share process context and can see each other's changes

Group parallel-safe steps together. Assign appropriate subagent types based on step nature.
"#;

/// Mode-specific context for Orchestration execution
const ORCHESTRATION_MODE_CONTEXT: &str = r#"
**Orchestration Mode**: Tasks delegated to external CLI agents in isolated git worktrees.
- Full process isolation per worker
- Worker types available:
  - claude: Claude Code CLI (best for complex reasoning)
  - gemini: Gemini CLI (fast for straightforward tasks)
  - safe-coder: Safe-Coder itself (recursive delegation)
  - copilot: GitHub Copilot CLI
- Each worker gets completely isolated workspace
- Changes are merged back after completion
- Best for: Large tasks with independent modules

Maximize parallelism by grouping independent work. Each step in a group runs concurrently.
"#;

/// System prompt template for mode-aware planning
const PLANNING_SYSTEM_PROMPT: &str = r#"You are a task planning expert for Safe-Coder, an AI coding assistant. Your job is to create execution plans optimized for the specified mode.

## Execution Mode: {mode}

{mode_context}

## Planning Guidelines

1. **Group Parallel Work**: Steps that can run simultaneously should be in the same group
2. **Respect Dependencies**: Groups execute in order; a group waits for all previous groups to complete
3. **Right-size Steps**: Each step should be independently completable and focused
4. **Assign Executors**: Based on step complexity and what the mode supports
5. **Be Specific**: Include relevant file paths and clear instructions

## Complexity Scoring (0-100)
- 0-30: Simple (single file, small change, documentation) -> inline or analyzer
- 31-60: Medium (multi-file, new feature, tests) -> subagent (tester/refactorer) or worker
- 61-100: Complex (architectural, cross-cutting, refactoring) -> worker with isolation

## Output Format

Respond with ONLY a valid JSON object (no markdown, no explanation):

{
  "title": "Brief descriptive title for the plan",
  "groups": [
    {
      "id": "group-1",
      "depends_on": [],
      "steps": [
        {
          "id": "step-1",
          "description": "Imperative description (e.g., 'Add validation to form')",
          "instructions": "Detailed instructions for completing this step",
          "relevant_files": ["path/to/file.rs", "path/to/other.rs"],
          "complexity_score": 25,
          "suggested_executor": "inline"
        }
      ]
    },
    {
      "id": "group-2",
      "depends_on": ["group-1"],
      "steps": [
        {
          "id": "step-2a",
          "description": "First parallel step",
          "instructions": "Instructions here",
          "relevant_files": ["file1.rs"],
          "complexity_score": 40,
          "suggested_executor": {"subagent": "tester"}
        },
        {
          "id": "step-2b",
          "description": "Second parallel step (runs with 2a)",
          "instructions": "Instructions here",
          "relevant_files": ["file2.rs"],
          "complexity_score": 45,
          "suggested_executor": {"subagent": "refactorer"}
        }
      ]
    }
  ]
}

For suggested_executor, use:
- "inline" for simple steps that run in the main session
- {"subagent": "analyzer"} for read-only code analysis
- {"subagent": "tester"} for test creation and running
- {"subagent": "refactorer"} for code improvements
- {"subagent": "documenter"} for documentation
- {"worker": "claude"} for complex isolated work
- {"worker": "gemini"} for quick parallel execution
- {"worker": "safe-coder"} for recursive delegation
- {"worker": "copilot"} for copilot-assisted work
"#;

/// The unified planner
pub struct UnifiedPlanner {
    /// Execution mode to plan for
    execution_mode: ExecutionMode,
}

impl UnifiedPlanner {
    /// Create a new planner for the specified execution mode
    pub fn new(mode: ExecutionMode) -> Self {
        Self {
            execution_mode: mode,
        }
    }

    /// Create a unified plan from a user request
    pub async fn create_plan(
        &self,
        llm_client: &dyn LlmClient,
        request: &str,
        project_context: Option<&str>,
    ) -> Result<UnifiedPlan> {
        // Build mode-aware system prompt
        let system_prompt = self.build_system_prompt();

        // Build user message with optional project context
        let user_message = if let Some(ctx) = project_context {
            format!(
                "Create an execution plan for the following request:\n\n{}\n\n## Project Context\n\n{}",
                request, ctx
            )
        } else {
            format!(
                "Create an execution plan for the following request:\n\n{}",
                request
            )
        };

        // Call LLM
        let response = llm_client
            .send_message_with_system(&[Message::user(user_message)], &[], Some(&system_prompt))
            .await
            .context("Failed to get planning response from LLM")?;

        // Extract text from response
        let response_text = response
            .message
            .content
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text { text } = block {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        // Parse response
        let plan_response = self.parse_response(&response_text)?;

        // Build plan
        self.build_plan(request, plan_response)
    }

    /// Build the system prompt with mode-specific context
    fn build_system_prompt(&self) -> String {
        let mode_context = match self.execution_mode {
            ExecutionMode::Direct => DIRECT_MODE_CONTEXT,
            ExecutionMode::Subagent => SUBAGENT_MODE_CONTEXT,
            ExecutionMode::Orchestration => ORCHESTRATION_MODE_CONTEXT,
        };

        PLANNING_SYSTEM_PROMPT
            .replace("{mode}", &format!("{:?}", self.execution_mode))
            .replace("{mode_context}", mode_context)
    }

    /// Parse the LLM response JSON
    fn parse_response(&self, response: &str) -> Result<PlanResponse> {
        // Try to extract JSON from the response
        let json_str = self.extract_json(response);

        serde_json::from_str(&json_str).context("Failed to parse planning response as JSON")
    }

    /// Extract JSON from response (handles markdown code blocks)
    fn extract_json(&self, response: &str) -> String {
        let trimmed = response.trim();

        // Check for markdown code block
        if trimmed.starts_with("```") {
            // Find the start of JSON (after ```json or ```)
            let start = if let Some(pos) = trimmed.find('\n') {
                pos + 1
            } else {
                3
            };

            // Find the end (before closing ```)
            if let Some(end) = trimmed.rfind("```") {
                if end > start {
                    return trimmed[start..end].trim().to_string();
                }
            }
        }

        // Try to find JSON object directly
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }

        // Return as-is if no extraction needed
        trimmed.to_string()
    }

    /// Build a UnifiedPlan from the parsed response
    fn build_plan(&self, request: &str, response: PlanResponse) -> Result<UnifiedPlan> {
        let plan_id = format!("plan-{}", &Uuid::new_v4().to_string()[..8]);

        let groups = response
            .groups
            .into_iter()
            .map(|g| {
                let steps = g
                    .steps
                    .into_iter()
                    .map(|s| {
                        UnifiedStep::new(&s.id, &s.description)
                            .with_instructions(&s.instructions)
                            .with_files(s.relevant_files)
                            .with_complexity(s.complexity_score)
                            .with_executor(self.parse_executor(&s.suggested_executor))
                    })
                    .collect();

                StepGroup {
                    id: g.id,
                    depends_on: g.depends_on,
                    steps,
                }
            })
            .collect();

        let mut plan = UnifiedPlan::new(&plan_id, request)
            .with_title(&response.title)
            .with_mode(self.execution_mode)
            .with_groups(groups);

        plan.status = PlanStatus::Ready;

        Ok(plan)
    }

    /// Parse executor specification from LLM response
    fn parse_executor(&self, spec: &serde_json::Value) -> StepExecutor {
        match spec {
            serde_json::Value::String(s) if s == "inline" => StepExecutor::Inline,
            serde_json::Value::Object(obj) => {
                if let Some(kind) = obj.get("subagent").and_then(|v| v.as_str()) {
                    StepExecutor::Subagent {
                        kind: match kind {
                            "tester" => SubagentKind::Tester,
                            "refactorer" => SubagentKind::Refactorer,
                            "documenter" => SubagentKind::Documenter,
                            "analyzer" | "code_analyzer" => SubagentKind::CodeAnalyzer,
                            _ => SubagentKind::Custom,
                        },
                    }
                } else if let Some(kind) = obj.get("worker").and_then(|v| v.as_str()) {
                    StepExecutor::Worker {
                        kind: match kind {
                            "claude" | "claude_code" => WorkerKind::ClaudeCode,
                            "gemini" | "gemini_cli" => WorkerKind::GeminiCli,
                            "safe-coder" | "safe_coder" => WorkerKind::SafeCoder,
                            "copilot" | "github_copilot" => WorkerKind::GitHubCopilot,
                            _ => WorkerKind::ClaudeCode,
                        },
                    }
                } else {
                    StepExecutor::Inline
                }
            }
            _ => StepExecutor::Inline,
        }
    }
}

/// Response structure from LLM
#[derive(Debug, Deserialize)]
struct PlanResponse {
    title: String,
    groups: Vec<GroupResponse>,
}

#[derive(Debug, Deserialize)]
struct GroupResponse {
    id: String,
    #[serde(default)]
    depends_on: Vec<String>,
    steps: Vec<StepResponse>,
}

#[derive(Debug, Deserialize)]
struct StepResponse {
    id: String,
    description: String,
    #[serde(default)]
    instructions: String,
    #[serde(default)]
    relevant_files: Vec<String>,
    #[serde(default)]
    complexity_score: u8,
    #[serde(default = "default_executor")]
    suggested_executor: serde_json::Value,
}

fn default_executor() -> serde_json::Value {
    serde_json::Value::String("inline".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_plain() {
        let planner = UnifiedPlanner::new(ExecutionMode::Direct);
        let input = r#"{"title": "Test", "groups": []}"#;
        let result = planner.extract_json(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_extract_json_markdown() {
        let planner = UnifiedPlanner::new(ExecutionMode::Direct);
        let input = r#"```json
{"title": "Test", "groups": []}
```"#;
        let result = planner.extract_json(input);
        assert_eq!(result, r#"{"title": "Test", "groups": []}"#);
    }

    #[test]
    fn test_extract_json_with_text() {
        let planner = UnifiedPlanner::new(ExecutionMode::Direct);
        let input = r#"Here's the plan:
{"title": "Test", "groups": []}
That's the plan!"#;
        let result = planner.extract_json(input);
        assert_eq!(result, r#"{"title": "Test", "groups": []}"#);
    }

    #[test]
    fn test_parse_executor_inline() {
        let planner = UnifiedPlanner::new(ExecutionMode::Direct);
        let spec = serde_json::json!("inline");
        let result = planner.parse_executor(&spec);
        assert!(matches!(result, StepExecutor::Inline));
    }

    #[test]
    fn test_parse_executor_subagent() {
        let planner = UnifiedPlanner::new(ExecutionMode::Subagent);
        let spec = serde_json::json!({"subagent": "tester"});
        let result = planner.parse_executor(&spec);
        assert!(matches!(
            result,
            StepExecutor::Subagent {
                kind: SubagentKind::Tester
            }
        ));
    }

    #[test]
    fn test_parse_executor_worker() {
        let planner = UnifiedPlanner::new(ExecutionMode::Orchestration);
        let spec = serde_json::json!({"worker": "claude"});
        let result = planner.parse_executor(&spec);
        assert!(matches!(
            result,
            StepExecutor::Worker {
                kind: WorkerKind::ClaudeCode
            }
        ));
    }

    #[test]
    fn test_build_system_prompt_direct() {
        let planner = UnifiedPlanner::new(ExecutionMode::Direct);
        let prompt = planner.build_system_prompt();
        assert!(prompt.contains("Direct Mode"));
        assert!(prompt.contains("No parallelism available"));
    }

    #[test]
    fn test_build_system_prompt_orchestration() {
        let planner = UnifiedPlanner::new(ExecutionMode::Orchestration);
        let prompt = planner.build_system_prompt();
        assert!(prompt.contains("Orchestration Mode"));
        assert!(prompt.contains("git worktrees"));
    }
}
