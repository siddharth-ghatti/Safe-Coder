//! Sidebar state and types for OpenCode-style UI
//!
//! Displays task info, plan progress, token usage, and connection status.

use crate::planning::{PlanEvent, PlanStatus, PlanStepStatus, TaskPlan};
use crate::tools::todo::TodoItem;

/// Sidebar visibility and content state
#[derive(Debug, Clone)]
pub struct SidebarState {
    /// Whether sidebar is visible (default: true)
    pub visible: bool,
    /// Current task title/description
    pub current_task: Option<String>,
    /// Active plan display info (from tool executions)
    pub active_plan: Option<PlanDisplay>,
    /// Todo list display (from TodoWrite tool) - always shown as checklist
    pub todo_plan: Option<TodoPlanDisplay>,
    /// Tool execution steps (for build mode)
    pub tool_steps: Vec<ToolStepDisplay>,
    /// Scroll offset for tool steps (0 = show most recent)
    pub tool_steps_scroll_offset: usize,
    /// Token usage tracking
    pub token_usage: TokenUsage,
    /// LSP connection status
    pub connections: ConnectionStatus,
    /// Modified files in this session
    pub modified_files: Vec<ModifiedFile>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            visible: true, // Visible by default
            current_task: None,
            active_plan: None,
            todo_plan: None,
            tool_steps: Vec::new(),
            tool_steps_scroll_offset: 0,
            // Default to Claude's 200K context window
            token_usage: TokenUsage::with_context_window(200_000),
            connections: ConnectionStatus::default(),
            modified_files: Vec::new(),
        }
    }
}

impl SidebarState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with a specific context window size (from config)
    pub fn with_context_window(context_window: usize) -> Self {
        Self {
            token_usage: TokenUsage::with_context_window(context_window),
            ..Default::default()
        }
    }

    /// Toggle sidebar visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Set current task
    pub fn set_task(&mut self, task: String) {
        self.current_task = Some(task);
    }

    /// Clear current task
    pub fn clear_task(&mut self) {
        self.current_task = None;
        self.active_plan = None;
        // Note: We don't clear todo_plan here as todos persist across tasks
    }

    /// Update todo plan from current todo list
    pub fn update_todos(&mut self, todos: &[TodoItem]) {
        if todos.is_empty() {
            self.todo_plan = None;
        } else {
            self.todo_plan = Some(TodoPlanDisplay::from_todos(todos));
        }
    }

    /// Clear todo plan
    pub fn clear_todos(&mut self) {
        self.todo_plan = None;
    }

    /// Update diagnostic counts from LSP
    pub fn update_diagnostics(&mut self, errors: usize, warnings: usize) {
        self.connections.diagnostic_counts = (errors, warnings);
    }

    /// Get the current task text for shimmer display
    /// Priority: in_progress todo active_form > todo content > current_task (user query)
    pub fn current_task_active_form(&self) -> Option<String> {
        // First try to get in_progress todo item's active form
        if let Some(plan) = &self.todo_plan {
            if let Some(item) = plan.items.iter().find(|item| item.status == "in_progress") {
                if !item.active_form.is_empty() {
                    return Some(item.active_form.clone());
                } else if !item.content.is_empty() {
                    return Some(format!("Working on: {}", item.content));
                }
            }
        }

        // Fall back to current_task (the user's original query)
        // Truncate long queries and add "Working on:" prefix
        self.current_task.as_ref().map(|task| {
            let truncated = if task.len() > 40 {
                format!("{}...", &task[..37])
            } else {
                task.clone()
            };
            format!("Working on: {}", truncated)
        })
    }

    /// Update from a plan event
    pub fn update_from_event(&mut self, event: &PlanEvent) {
        match event {
            PlanEvent::PlanCreated { plan } => {
                // Don't overwrite current_task - keep the user's original query
                // The plan title is shown in the PLAN section instead

                // IMPORTANT: Don't replace an approved plan that's executing
                // When transitioning from PLAN to BUILD mode, the session sends a new
                // PlanCreated with tool calls as steps - but we want to keep the original plan
                if let Some(ref existing) = self.active_plan {
                    // If we already have a plan that's approved (not awaiting) and not completed,
                    // it means we're executing it - don't replace with tool-based plan
                    if !existing.awaiting_approval && !existing.completed {
                        // Plan is executing - don't replace it
                        return;
                    }
                }
                self.active_plan = Some(PlanDisplay::from_plan(plan));
            }
            PlanEvent::StepsAdded { steps, .. } => {
                // Add new steps to the existing plan
                if let Some(ref mut display) = self.active_plan {
                    for step in steps {
                        display.steps.push(PlanStepDisplay {
                            id: step.id.clone(),
                            description: step.description.clone(),
                            active_description: step.active_description.clone(),
                            status: step.status,
                        });
                    }
                }
            }
            PlanEvent::StepStarted {
                step_id,
                description,
                ..
            } => {
                if let Some(ref mut display) = self.active_plan {
                    display.set_step_status(step_id, PlanStepStatus::InProgress);
                    display.current_step_description = Some(description.clone());
                }
            }
            PlanEvent::StepProgress { message, .. } => {
                if let Some(ref mut display) = self.active_plan {
                    display.progress_message = Some(message.clone());
                }
            }
            PlanEvent::StepCompleted {
                step_id, success, ..
            } => {
                if let Some(ref mut display) = self.active_plan {
                    let status = if *success {
                        PlanStepStatus::Completed
                    } else {
                        PlanStepStatus::Failed
                    };
                    display.set_step_status(step_id, status);
                    display.current_step_description = None;
                    display.progress_message = None;
                }
            }
            PlanEvent::PlanCompleted { success, .. } => {
                if let Some(ref mut display) = self.active_plan {
                    display.completed = true;
                    display.success = Some(*success);
                }
            }
            PlanEvent::AwaitingApproval { .. } => {
                if let Some(ref mut display) = self.active_plan {
                    display.awaiting_approval = true;
                }
            }
            PlanEvent::PlanApproved { .. } => {
                if let Some(ref mut display) = self.active_plan {
                    display.awaiting_approval = false;
                }
            }
            PlanEvent::PlanRejected { .. } => {
                self.active_plan = None;
            }
        }
    }

    /// Update token usage
    pub fn update_tokens(&mut self, input: usize, output: usize) {
        self.token_usage.input_tokens += input;
        self.token_usage.output_tokens += output;
        self.token_usage.total_tokens =
            self.token_usage.input_tokens + self.token_usage.output_tokens;
    }

    /// Update token usage with cache information
    pub fn update_tokens_with_cache(
        &mut self,
        input: usize,
        output: usize,
        cache_read: Option<usize>,
        cache_write: Option<usize>,
    ) {
        self.token_usage.input_tokens += input;
        self.token_usage.output_tokens += output;
        self.token_usage.total_tokens =
            self.token_usage.input_tokens + self.token_usage.output_tokens;

        // Update cache stats if present
        let read_tokens = cache_read.unwrap_or(0);
        let write_tokens = cache_write.unwrap_or(0);
        if read_tokens > 0 || write_tokens > 0 {
            let is_hit = read_tokens > 0;
            self.token_usage
                .update_cache_stats(read_tokens, write_tokens, is_hit);
        }
    }

    /// Reset token usage (for new session)
    pub fn reset_tokens(&mut self) {
        self.token_usage = TokenUsage::default();
    }

    /// Add LSP server connection
    pub fn add_lsp_server(&mut self, name: String, connected: bool) {
        // Update existing or add new
        if let Some(server) = self
            .connections
            .lsp_servers
            .iter_mut()
            .find(|(n, _)| n == &name)
        {
            server.1 = connected;
        } else {
            self.connections.lsp_servers.push((name, connected));
        }
    }

    /// Remove LSP server
    pub fn remove_lsp_server(&mut self, name: &str) {
        self.connections.lsp_servers.retain(|(n, _)| n != name);
    }

    /// Track a file modification
    pub fn track_file_modification(&mut self, path: String, mod_type: ModificationType) {
        // Check if we already have this file
        if let Some(existing) = self.modified_files.iter_mut().find(|f| f.path == path) {
            // Update the modification type and timestamp
            existing.modification_type = mod_type;
            existing.timestamp = chrono::Local::now();
        } else {
            // Add new file
            self.modified_files.push(ModifiedFile {
                path,
                modification_type: mod_type,
                timestamp: chrono::Local::now(),
            });
        }
    }

    /// Get count of modified files
    pub fn modified_files_count(&self) -> usize {
        self.modified_files.len()
    }

    /// Clear modified files list
    pub fn clear_modified_files(&mut self) {
        self.modified_files.clear();
    }

    /// Add a tool step to the execution list
    pub fn add_tool_step(&mut self, tool_name: String, description: String) {
        let step = ToolStepDisplay {
            id: format!("tool-{}", self.tool_steps.len() + 1),
            tool_name,
            description,
            status: ToolStepStatus::Running,
            timestamp: chrono::Local::now(),
        };
        self.tool_steps.push(step);
        // Reset scroll to show the new step
        self.tool_steps_scroll_offset = 0;
    }

    /// Complete a tool step
    pub fn complete_tool_step(&mut self, tool_name: &str, success: bool) {
        if let Some(step) = self
            .tool_steps
            .iter_mut()
            .rev()
            .find(|s| s.tool_name == tool_name)
        {
            step.status = if success {
                ToolStepStatus::Completed
            } else {
                ToolStepStatus::Failed
            };
        }
    }

    /// Clear tool steps (for new session or task)
    pub fn clear_tool_steps(&mut self) {
        self.tool_steps.clear();
        self.tool_steps_scroll_offset = 0;
    }

    /// Get count of completed tool steps
    pub fn completed_tool_steps(&self) -> usize {
        self.tool_steps
            .iter()
            .filter(|s| s.status == ToolStepStatus::Completed)
            .count()
    }

    /// Scroll tool steps up (towards older steps)
    pub fn scroll_tool_steps_up(&mut self) {
        if self.tool_steps_scroll_offset < self.tool_steps.len().saturating_sub(1) {
            self.tool_steps_scroll_offset += 1;
        }
    }

    /// Scroll tool steps down (towards newer steps)
    pub fn scroll_tool_steps_down(&mut self) {
        self.tool_steps_scroll_offset = self.tool_steps_scroll_offset.saturating_sub(1);
    }

    /// Reset scroll to show most recent steps
    pub fn reset_tool_steps_scroll(&mut self) {
        self.tool_steps_scroll_offset = 0;
    }
}

/// Display representation of a plan for the sidebar
#[derive(Debug, Clone)]
pub struct PlanDisplay {
    /// Plan title
    pub title: String,
    /// Steps with their display info
    pub steps: Vec<PlanStepDisplay>,
    /// Index of currently executing step
    pub current_step_idx: Option<usize>,
    /// Current step description (active form)
    pub current_step_description: Option<String>,
    /// Progress message for current step
    pub progress_message: Option<String>,
    /// Whether plan is awaiting approval
    pub awaiting_approval: bool,
    /// Whether plan is completed
    pub completed: bool,
    /// Success status if completed
    pub success: Option<bool>,
}

impl PlanDisplay {
    pub fn from_plan(plan: &TaskPlan) -> Self {
        let steps = plan
            .steps
            .iter()
            .map(|s| PlanStepDisplay {
                id: s.id.clone(),
                description: s.description.clone(),
                active_description: s.active_description.clone(),
                status: s.status,
            })
            .collect();

        Self {
            title: plan.title.clone(),
            steps,
            current_step_idx: plan.current_step_index(),
            current_step_description: None,
            progress_message: None,
            awaiting_approval: plan.status == PlanStatus::AwaitingApproval,
            completed: plan.status.is_terminal(),
            success: if plan.status == PlanStatus::Completed {
                Some(true)
            } else if plan.status == PlanStatus::Failed {
                Some(false)
            } else {
                None
            },
        }
    }

    /// Update step status by ID
    pub fn set_step_status(&mut self, step_id: &str, status: PlanStepStatus) {
        if let Some(step) = self.steps.iter_mut().find(|s| s.id == step_id) {
            step.status = status;
            if status == PlanStepStatus::InProgress {
                self.current_step_idx = self.steps.iter().position(|s| s.id == step_id);
            }
        }
    }

    /// Get count of completed steps
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == PlanStepStatus::Completed)
            .count()
    }

    /// Get progress as percentage
    pub fn progress_percent(&self) -> f32 {
        if self.steps.is_empty() {
            0.0
        } else {
            (self.completed_count() as f32 / self.steps.len() as f32) * 100.0
        }
    }
}

/// Display info for a single plan step
#[derive(Debug, Clone)]
pub struct PlanStepDisplay {
    /// Step ID
    pub id: String,
    /// Imperative description
    pub description: String,
    /// Active/continuous description
    pub active_description: String,
    /// Current status
    pub status: PlanStepStatus,
}

impl PlanStepDisplay {
    /// Get the display icon for this step
    pub fn icon(&self) -> &'static str {
        self.status.icon()
    }
}

/// Display representation of todo items as a plan checklist
#[derive(Debug, Clone)]
pub struct TodoPlanDisplay {
    /// Title for the todo section
    pub title: String,
    /// Todo items as steps
    pub items: Vec<TodoItemDisplay>,
}

impl TodoPlanDisplay {
    /// Create from a list of TodoItems
    pub fn from_todos(todos: &[TodoItem]) -> Self {
        let items = todos
            .iter()
            .enumerate()
            .map(|(i, todo)| TodoItemDisplay {
                id: format!("todo-{}", i + 1),
                content: todo.content.clone(),
                active_form: todo.active_form.clone(),
                status: todo.status.clone(),
            })
            .collect();

        Self {
            title: "Tasks".to_string(),
            items,
        }
    }

    /// Get count of completed items
    pub fn completed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == "completed")
            .count()
    }

    /// Get progress as percentage
    pub fn progress_percent(&self) -> f32 {
        if self.items.is_empty() {
            0.0
        } else {
            (self.completed_count() as f32 / self.items.len() as f32) * 100.0
        }
    }

    /// Check if any item is in progress
    pub fn has_in_progress(&self) -> bool {
        self.items.iter().any(|i| i.status == "in_progress")
    }
}

/// Display info for a single todo item
#[derive(Debug, Clone)]
pub struct TodoItemDisplay {
    /// Item ID
    pub id: String,
    /// Content/description
    pub content: String,
    /// Active form (present tense)
    pub active_form: String,
    /// Status: pending, in_progress, completed
    pub status: String,
}

impl TodoItemDisplay {
    /// Get the display icon for this item
    pub fn icon(&self) -> &'static str {
        match self.status.as_str() {
            "completed" => "✓",
            "in_progress" => "◐", // Will be animated in UI
            "pending" => "◯",
            _ => "?",
        }
    }
}

/// Display representation of a tool execution step
#[derive(Debug, Clone)]
pub struct ToolStepDisplay {
    /// Step ID
    pub id: String,
    /// Name of the tool being executed
    pub tool_name: String,
    /// Description of what the tool is doing
    pub description: String,
    /// Current status
    pub status: ToolStepStatus,
    /// When the step was started
    pub timestamp: chrono::DateTime<chrono::Local>,
}

/// Status of a tool execution step
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolStepStatus {
    /// Tool is currently running
    Running,
    /// Tool completed successfully
    Completed,
    /// Tool failed
    Failed,
}

impl ToolStepStatus {
    /// Get the display icon for this status
    pub fn icon(&self) -> &'static str {
        match self {
            ToolStepStatus::Running => "◐", // Will be animated in UI
            ToolStepStatus::Completed => "✓",
            ToolStepStatus::Failed => "✗",
        }
    }
}

/// Token usage tracking
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    /// Tokens used for input (prompts)
    pub input_tokens: usize,
    /// Tokens used for output (responses)
    pub output_tokens: usize,
    /// Total tokens used
    pub total_tokens: usize,
    /// Context window size (model dependent)
    pub context_window: usize,
    /// Compaction threshold percentage (default 60%)
    pub compact_threshold_pct: usize,
    /// Tokens that have been compressed/summarized (to show history)
    pub compressed_tokens: usize,
    /// Tokens read from provider cache (Anthropic/OpenAI)
    pub cache_read_tokens: usize,
    /// Tokens written to provider cache
    pub cache_write_tokens: usize,
    /// Number of cache hits
    pub cache_hits: usize,
    /// Number of cache misses
    pub cache_misses: usize,
    /// Estimated cost savings from caching (in dollars)
    pub estimated_savings: f64,
}

impl TokenUsage {
    /// Create with a specific context window size
    pub fn with_context_window(context_window: usize) -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            context_window,
            compact_threshold_pct: 60, // Default to 60%
            compressed_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cache_hits: 0,
            cache_misses: 0,
            estimated_savings: 0.0,
        }
    }

    /// Calculate context left until auto-compact (as percentage points)
    /// Returns how many percentage points of context remain before compaction triggers
    pub fn context_left_until_compact(&self) -> usize {
        let current_pct = self.usage_percent() as usize;
        if current_pct >= self.compact_threshold_pct {
            0
        } else {
            self.compact_threshold_pct - current_pct
        }
    }

    /// Record that tokens were compressed
    pub fn record_compression(&mut self, tokens_compressed: usize) {
        self.compressed_tokens += tokens_compressed;
    }

    /// Calculate compressed usage percentage (for the secondary bar)
    pub fn compressed_percent(&self) -> f32 {
        if self.context_window == 0 {
            0.0
        } else {
            (self.compressed_tokens as f32 / self.context_window as f32) * 100.0
        }
    }

    /// Get total tokens including compressed (for display purposes)
    pub fn total_with_compressed(&self) -> usize {
        self.total_tokens + self.compressed_tokens
    }

    /// Calculate usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.context_window == 0 {
            0.0
        } else {
            (self.total_tokens as f32 / self.context_window as f32) * 100.0
        }
    }

    /// Format for display
    pub fn format_display(&self) -> String {
        format!(
            "In: {} / Out: {}",
            format_number(self.input_tokens),
            format_number(self.output_tokens)
        )
    }

    /// Format detailed breakdown
    pub fn format_detailed(&self) -> String {
        format!(
            "In: {} / Out: {} / Total: {}",
            format_number(self.input_tokens),
            format_number(self.output_tokens),
            format_number(self.total_tokens)
        )
    }

    /// Update cache statistics from LLM response
    pub fn update_cache_stats(&mut self, cache_read: usize, cache_write: usize, is_hit: bool) {
        self.cache_read_tokens += cache_read;
        self.cache_write_tokens += cache_write;
        if is_hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
        self.update_estimated_savings();
    }

    /// Calculate cache hit rate as percentage
    pub fn cache_hit_rate(&self) -> f32 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f32 / total as f32) * 100.0
        }
    }

    /// Update estimated savings based on cache reads
    /// Uses approximate pricing: cache reads are 90% cheaper than regular input
    fn update_estimated_savings(&mut self) {
        // Anthropic pricing: ~$3/1M input tokens, cached = $0.30/1M (90% savings)
        // We save $2.70 per 1M cached tokens
        let savings_per_million = 2.70;
        self.estimated_savings =
            (self.cache_read_tokens as f64 / 1_000_000.0) * savings_per_million;
    }

    /// Format cache display for sidebar
    pub fn format_cache_display(&self) -> String {
        if self.cache_read_tokens == 0 && self.cache_write_tokens == 0 {
            "Cache: --".to_string()
        } else {
            format!(
                "Cache: {} read ({:.0}% hit)",
                format_number(self.cache_read_tokens),
                self.cache_hit_rate()
            )
        }
    }

    /// Format savings display
    pub fn format_savings(&self) -> String {
        if self.estimated_savings < 0.01 {
            "Saved: <$0.01".to_string()
        } else {
            format!("Saved: ~${:.2}", self.estimated_savings)
        }
    }

    /// Check if there's any cache activity to display
    pub fn has_cache_activity(&self) -> bool {
        self.cache_read_tokens > 0 || self.cache_write_tokens > 0 || self.cache_hits > 0
    }
}

/// Format large numbers with K/M suffixes
fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Connection status for external services
#[derive(Debug, Clone, Default)]
pub struct ConnectionStatus {
    /// LSP servers (name, connected)
    pub lsp_servers: Vec<(String, bool)>,
    /// Current diagnostic counts (errors, warnings)
    pub diagnostic_counts: (usize, usize),
}

/// A file that was modified during the session
#[derive(Debug, Clone)]
pub struct ModifiedFile {
    /// Path to the file (relative to project root)
    pub path: String,
    /// Type of modification
    pub modification_type: ModificationType,
    /// When the file was last modified
    pub timestamp: chrono::DateTime<chrono::Local>,
}

/// Type of file modification
#[derive(Debug, Clone, PartialEq)]
pub enum ModificationType {
    /// File was created
    Created,
    /// File was edited
    Edited,
    /// File was deleted
    Deleted,
}

impl ConnectionStatus {
    /// Check if any LSP servers are connected
    pub fn has_connected_lsp(&self) -> bool {
        self.lsp_servers.iter().any(|(_, connected)| *connected)
    }

    /// Get count of connected LSP servers
    pub fn connected_lsp_count(&self) -> usize {
        self.lsp_servers.iter().filter(|(_, c)| *c).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sidebar_toggle() {
        let mut sidebar = SidebarState::new();
        assert!(sidebar.visible);
        sidebar.toggle();
        assert!(!sidebar.visible);
        sidebar.toggle();
        assert!(sidebar.visible);
    }

    #[test]
    fn test_token_usage_format() {
        let usage = TokenUsage {
            input_tokens: 1500,
            output_tokens: 500,
            total_tokens: 2000,
            context_window: 200_000,
            compact_threshold_pct: 60,
            compressed_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cache_hits: 0,
            cache_misses: 0,
            estimated_savings: 0.0,
        };
        assert_eq!(usage.format_display(), "In: 1.5K / Out: 500");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(500), "500");
        assert_eq!(format_number(1500), "1.5K");
        assert_eq!(format_number(1_500_000), "1.5M");
    }

    #[test]
    fn test_plan_display_progress() {
        let mut display = PlanDisplay {
            title: "Test".to_string(),
            steps: vec![
                PlanStepDisplay {
                    id: "1".to_string(),
                    description: "Step 1".to_string(),
                    active_description: "Stepping 1".to_string(),
                    status: PlanStepStatus::Completed,
                },
                PlanStepDisplay {
                    id: "2".to_string(),
                    description: "Step 2".to_string(),
                    active_description: "Stepping 2".to_string(),
                    status: PlanStepStatus::InProgress,
                },
                PlanStepDisplay {
                    id: "3".to_string(),
                    description: "Step 3".to_string(),
                    active_description: "Stepping 3".to_string(),
                    status: PlanStepStatus::Pending,
                },
            ],
            current_step_idx: Some(1),
            current_step_description: None,
            progress_message: None,
            awaiting_approval: false,
            completed: false,
            success: None,
        };

        assert_eq!(display.completed_count(), 1);
        assert!((display.progress_percent() - 33.33).abs() < 1.0);
    }
}
