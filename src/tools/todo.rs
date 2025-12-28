use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use super::{Tool, ToolContext};

/// A single todo item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    /// The content/description of the todo
    pub content: String,
    /// Status: "pending", "in_progress", or "completed"
    pub status: String,
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
                    "description": "The complete list of todos to set",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": {
                                "type": "string",
                                "description": "The task description"
                            },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "The task status"
                            },
                            "priority": {
                                "type": "integer",
                                "minimum": 1,
                                "maximum": 5,
                                "description": "Priority 1 (highest) to 5 (lowest). Defaults to 3."
                            }
                        },
                        "required": ["content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext<'_>) -> Result<String> {
        let params: TodoWriteParams = serde_json::from_value(params)?;

        let mut todo_list = TODO_LIST.lock().unwrap();
        *todo_list = params.todos;

        let pending = todo_list.iter().filter(|t| t.status == "pending").count();
        let in_progress = todo_list
            .iter()
            .filter(|t| t.status == "in_progress")
            .count();
        let completed = todo_list.iter().filter(|t| t.status == "completed").count();

        Ok(format!(
            "Todo list updated: {} total ({} pending, {} in progress, {} completed)",
            todo_list.len(),
            pending,
            in_progress,
            completed
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
