//! Sidebar state and types for OpenCode-style UI
//!
//! Displays task info, plan progress, token usage, and connection status.

use crate::planning::{PlanEvent, PlanStatus, PlanStepStatus, TaskPlan};

/// Sidebar visibility and content state
#[derive(Debug, Clone)]
pub struct SidebarState {
    /// Whether sidebar is visible (default: true)
    pub visible: bool,
    /// Current task title/description
    pub current_task: Option<String>,
    /// Active plan display info
    pub active_plan: Option<PlanDisplay>,
    /// Token usage tracking
    pub token_usage: TokenUsage,
    /// LSP connection status
    pub connections: ConnectionStatus,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            visible: true, // Visible by default
            current_task: None,
            active_plan: None,
            token_usage: TokenUsage::default(),
            connections: ConnectionStatus::default(),
        }
    }
}

impl SidebarState {
    pub fn new() -> Self {
        Self::default()
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
    }

    /// Update from a plan event
    pub fn update_from_event(&mut self, event: &PlanEvent) {
        match event {
            PlanEvent::PlanCreated { plan } => {
                self.current_task = Some(plan.title.clone());
                self.active_plan = Some(PlanDisplay::from_plan(plan));
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
}

impl TokenUsage {
    /// Create with a specific context window size
    pub fn with_context_window(context_window: usize) -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            context_window,
        }
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
        if self.context_window > 0 {
            format!(
                "{} tokens ({:.0}%)",
                format_number(self.total_tokens),
                self.usage_percent()
            )
        } else {
            format!("{} tokens", format_number(self.total_tokens))
        }
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
        };
        assert_eq!(usage.format_display(), "2.0K tokens (1%)");
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
