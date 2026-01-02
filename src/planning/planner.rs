//! LLM-based task planner
//!
//! Uses the LLM to decompose user tasks into structured plans.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::llm::{ContentBlock, LlmClient, Message};
use crate::tools::AgentMode;

use super::complexity::score_and_assign_plan;
use super::types::{PlanStep, TaskPlan};

/// System prompt for detailed planning (PLAN mode)
const DETAILED_PLANNING_PROMPT: &str = r#"You are a task planning expert. Break down coding tasks into clear, actionable steps.

Your job is to analyze the user's request and create a structured plan with steps that can be executed independently.

## Guidelines:
1. Each step should be independently completable
2. Order steps by dependencies (analysis → implementation → testing → documentation)
3. Keep steps focused - one logical change per step
4. Identify relevant files for each step
5. Provide detailed instructions for each step

## Complexity hints:
- Simple: Single file changes, documentation updates, small fixes
- Medium: Multi-file changes, new features, significant modifications
- Complex: Refactoring, architectural changes, cross-cutting concerns

## Output Format:
Respond with ONLY a JSON object (no markdown, no explanation):
{
  "title": "Brief plan title (max 50 chars)",
  "steps": [
    {
      "description": "Imperative description (e.g., Add validation to signup form)",
      "instructions": "Detailed step-by-step instructions for completing this step",
      "relevant_files": ["path/to/file.rs", "path/to/another.rs"]
    }
  ]
}

Important: Output ONLY the JSON object, nothing else."#;

/// System prompt for quick planning (BUILD mode)
const QUICK_PLANNING_PROMPT: &str = r#"You are a task planning expert. Quickly break down coding tasks into steps.

Create a brief plan with the key steps needed. Keep it concise - this will execute immediately.

## Guidelines:
1. 2-5 steps maximum
2. Focus on the essential actions
3. Keep descriptions brief
4. Identify key files

## Output Format:
Respond with ONLY a JSON object (no markdown, no explanation):
{
  "title": "Brief title",
  "steps": [
    {
      "description": "Brief action description",
      "instructions": "Key instructions",
      "relevant_files": ["path/to/file.rs"]
    }
  ]
}

Important: Output ONLY the JSON object, nothing else."#;

/// Response structure from LLM planning
#[derive(Debug, Deserialize, Serialize)]
struct PlanResponse {
    title: String,
    steps: Vec<StepResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
struct StepResponse {
    description: String,
    instructions: String,
    #[serde(default)]
    relevant_files: Vec<String>,
}

/// Task planner that uses LLM for decomposition
pub struct TaskPlanner {
    detailed: bool,
}

impl TaskPlanner {
    /// Create a planner for detailed planning (PLAN mode)
    pub fn detailed() -> Self {
        Self { detailed: true }
    }

    /// Create a planner for quick planning (BUILD mode)
    pub fn quick() -> Self {
        Self { detailed: false }
    }

    /// Create a planner based on agent mode
    pub fn for_mode(mode: AgentMode) -> Self {
        match mode {
            AgentMode::Plan => Self::detailed(),
            AgentMode::Build => Self::quick(),
        }
    }

    /// Create a plan from a user request
    pub async fn create_plan(
        &self,
        llm_client: &dyn LlmClient,
        request: &str,
        context: &str,
    ) -> Result<TaskPlan> {
        let plan_id = format!("plan-{}", &Uuid::new_v4().to_string()[..8]);
        let mut plan = TaskPlan::new(plan_id, request.to_string());

        // Build the user message with context
        let user_message = if context.is_empty() {
            format!("Create a plan for this task:\n\n{}", request)
        } else {
            format!(
                "Create a plan for this task:\n\n{}\n\nProject context:\n{}",
                request, context
            )
        };

        // Get system prompt based on mode
        let system_prompt = if self.detailed {
            DETAILED_PLANNING_PROMPT
        } else {
            QUICK_PLANNING_PROMPT
        };

        // Call LLM
        let messages = vec![Message::user(user_message)];
        let llm_response = llm_client
            .send_message_with_system(&messages, &[], Some(system_prompt))
            .await?;

        // Extract text from response
        let response_text = llm_response
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

        // Parse JSON response
        let plan_response: PlanResponse = Self::parse_plan_json(&response_text)?;

        // Convert to TaskPlan
        plan.title = plan_response.title;
        plan.steps = plan_response
            .steps
            .into_iter()
            .enumerate()
            .map(|(i, step)| {
                PlanStep::new(format!("step-{}", i + 1), step.description)
                    .with_instructions(step.instructions)
                    .with_files(step.relevant_files)
            })
            .collect();

        // Score complexity and assign subagents
        score_and_assign_plan(&mut plan);

        // Mark as ready
        plan.status = super::types::PlanStatus::Ready;

        Ok(plan)
    }

    /// Parse JSON from LLM response, handling potential markdown wrappers
    fn parse_plan_json(text: &str) -> Result<PlanResponse> {
        let text = text.trim();

        // Try to extract JSON from markdown code block if present
        let json_str = if text.starts_with("```") {
            // Find the JSON content between code fences
            let start = text.find('{').unwrap_or(0);
            let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
            &text[start..end]
        } else if text.starts_with('{') {
            text
        } else {
            // Try to find JSON object in the text
            let start = text.find('{').unwrap_or(0);
            let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
            &text[start..end]
        };

        serde_json::from_str(json_str).map_err(|e| {
            anyhow::anyhow!(
                "Failed to parse plan JSON: {}. Response was: {}",
                e,
                &text[..text.len().min(200)]
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plan_json_clean() {
        let json = r#"{"title": "Test Plan", "steps": [{"description": "Step 1", "instructions": "Do this", "relevant_files": []}]}"#;
        let result = TaskPlanner::parse_plan_json(json);
        assert!(result.is_ok());
        let plan = result.unwrap();
        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn test_parse_plan_json_with_markdown() {
        let json = r#"```json
{"title": "Test Plan", "steps": [{"description": "Step 1", "instructions": "Do this", "relevant_files": []}]}
```"#;
        let result = TaskPlanner::parse_plan_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_plan_json_with_prefix() {
        let json = r#"Here is the plan:
{"title": "Test Plan", "steps": [{"description": "Step 1", "instructions": "Do this", "relevant_files": []}]}"#;
        let result = TaskPlanner::parse_plan_json(json);
        assert!(result.is_ok());
    }
}
