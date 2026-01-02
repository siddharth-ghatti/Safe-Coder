use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use super::{Tool, ToolContext};

/// Maximum number of todo items allowed (prevents infinite task lists)
const MAX_TODO_ITEMS: usize = 20;

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// The content/description of the todo
    pub content: String,
    /// Status: "pending", "in_progress", or "completed"
    pub status: String,
    /// The active form of the task (present tense, e.g., "Adding tests...")
    #[serde(default)]
    pub active_form: String,
    /// Priority: 1 (highest) to 5 (lowest)
    #[serde(default = "default_priority")]
    pub priority: u8,
}

fn default_priority() -> u8 {
    3
}

/// Global todo list storage (shared across tool instances)
lazy_static::lazy_static! {
    static ref TODO_LIST: Arc<Mutex<Vec<TodoItem>>> = Arc::new(Mutex::new(Vec::new()));
    /// Track how many turns since last todo update (for soft reminders)
    static ref TURNS_WITHOUT_UPDATE: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
}

/// Get the current todo list (for sidebar display)
pub fn get_todo_list() -> Vec<TodoItem> {
    TODO_LIST.lock().unwrap().clone()
}

/// Increment turns without update counter (called by session after each LLM response)
pub fn increment_turns_without_update() {
    let mut turns = TURNS_WITHOUT_UPDATE.lock().unwrap();
    *turns += 1;
}

/// Get turns without update and check if reminder is needed
pub fn get_turns_without_update() -> usize {
    *TURNS_WITHOUT_UPDATE.lock().unwrap()
}

/// Check if a soft reminder should be shown (10+ turns without update)
pub fn should_show_reminder() -> bool {
    let turns = TURNS_WITHOUT_UPDATE.lock().unwrap();
    *turns >= 10
}

/// Reset turns counter (called when todos are updated)
fn reset_turns_counter() {
    let mut turns = TURNS_WITHOUT_UPDATE.lock().unwrap();
    *turns = 0;
}

// ============ TodoWrite Tool ============

#[derive(Debug, Deserialize)]
struct TodoWriteParams {
    /// The list of todos to set (replaces current list)
    todos: Vec<TodoItem>,
}

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todowrite"
    }

    fn description(&self) -> &str {
        "Write/update the todo list for tracking tasks during this session. \
         Use this to plan work, track progress, and organize complex tasks. \
         The todo list helps maintain context across multiple steps."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The complete list of todos to set (max 20 items, only 1 can be in_progress)",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The task description (imperative form, e.g., 'Add unit tests')"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "The task status. Only ONE task can be in_progress at a time."
                            },
                            "active_form": {
                                "type": "string",
                                "description": "Present tense form shown during execution (e.g., 'Adding unit tests...')"
                            },
                            "priority": {
                                "type": "integer",
                                "minimum": 1,
                                "maximum": 5,
                                "description": "Priority 1 (highest) to 5 (lowest). Defaults to 3."
                            }
                        },
                        "required": ["content", "status", "active_form"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext<'_>) -> Result<String> {
        let params: TodoWriteParams = serde_json::from_value(params)?;

        // Constraint 1: Max 20 items
        if params.todos.len() > MAX_TODO_ITEMS {
            return Ok(format!(
                "Error: Too many todos ({} items). Maximum allowed is {}. Please consolidate or remove some tasks.",
                params.todos.len(),
                MAX_TODO_ITEMS
            ));
        }

        // Constraint 2: Only one in_progress at a time
        let in_progress_count = params
            .todos
            .iter()
            .filter(|t| t.status == "in_progress")
            .count();
        if in_progress_count > 1 {
            return Ok(format!(
                "Error: {} tasks are marked as in_progress. Only ONE task can be in_progress at a time. \
                 Focus on completing one task before starting another.",
                in_progress_count
            ));
        }

        let mut todo_list = TODO_LIST.lock().unwrap();
        *todo_list = params.todos;

        // Reset the turns counter since todos were updated
        drop(todo_list); // Release lock before calling reset
        reset_turns_counter();

        let todo_list = TODO_LIST.lock().unwrap();
        let pending = todo_list.iter().filter(|t| t.status == "pending").count();
        let in_progress = todo_list
            .iter()
            .filter(|t| t.status == "in_progress")
            .count();
        let completed = todo_list.iter().filter(|t| t.status == "completed").count();

        // Show active task if one is in progress
        let active_msg = if in_progress == 1 {
            if let Some(active) = todo_list.iter().find(|t| t.status == "in_progress") {
                let form = if active.active_form.is_empty() {
                    &active.content
                } else {
                    &active.active_form
                };
                format!(" Currently: {}", form)
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        Ok(format!(
            "Todo list updated: {} total ({} pending, {} in progress, {} completed).{}",
            todo_list.len(),
            pending,
            in_progress,
            completed,
            active_msg
        ))
    }
}

// ============ TodoRead Tool ============

pub struct TodoReadTool;

#[async_trait]
impl Tool for TodoReadTool {
    fn name(&self) -> &str {
        "todoread"
    }

    fn description(&self) -> &str {
        "Read the current todo list to see task status and what needs to be done. \
         Use this to check progress and decide what to work on next."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext<'_>) -> Result<String> {
        let todo_list = TODO_LIST.lock().unwrap();

        if todo_list.is_empty() {
            return Ok("No todos in the list.".to_string());
        }

        let mut output = Vec::new();
        output.push(format!("📋 Todo List ({} items):", todo_list.len()));
        output.push("".to_string());

        for (idx, todo) in todo_list.iter().enumerate() {
            let status_icon = match todo.status.as_str() {
                "completed" => "✅",
                "in_progress" => "🔄",
                "pending" => "⬜",
                _ => "❓",
            };

            let priority_str = match todo.priority {
                1 => "🔴",
                2 => "🟠",
                3 => "🟡",
                4 => "🟢",
                5 => "⚪",
                _ => "⚪",
            };

            output.push(format!(
                "{}. {} {} [P{}] {}",
                idx + 1,
                status_icon,
                priority_str,
                todo.priority,
                todo.content
            ));
        }

        // Summary
        let pending = todo_list.iter().filter(|t| t.status == "pending").count();
        let in_progress = todo_list
            .iter()
            .filter(|t| t.status == "in_progress")
            .count();
        let completed = todo_list.iter().filter(|t| t.status == "completed").count();

        output.push("".to_string());
        output.push(format!(
            "Summary: {} pending | {} in progress | {} completed",
            pending, in_progress, completed
        ));

        Ok(output.join("\n"))
    }
}
