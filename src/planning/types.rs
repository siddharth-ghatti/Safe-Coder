//! Core types for task planning

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::subagent::SubagentKind;

/// Complexity level for a plan step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepComplexity {
    /// Simple task - execute inline in main session (score 0-30)
    Simple,
    /// Medium task - may benefit from subagent but can be inline (score 31-60)
    Medium,
    /// Complex task - should delegate to subagent (score 61-100)
    Complex,
}

impl StepComplexity {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepComplexity::Simple => "simple",
            StepComplexity::Medium => "medium",
            StepComplexity::Complex => "complex",
        }
    }
}

/// Assignment decision for a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepAssignment {
    /// Execute inline in the main session
    Inline,
    /// Delegate to a subagent
    Subagent { kind: SubagentKind, reason: String },
}

impl Default for StepAssignment {
    fn default() -> Self {
        StepAssignment::Inline
    }
}

/// Status of a plan step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlanStepStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl PlanStepStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            PlanStepStatus::Pending => "◯",
            PlanStepStatus::InProgress => "◐",
            PlanStepStatus::Completed => "✓",
            PlanStepStatus::Failed => "✗",
            PlanStepStatus::Skipped => "−",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PlanStepStatus::Completed | PlanStepStatus::Failed | PlanStepStatus::Skipped
        )
    }
}

/// A single step in a task plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    /// Unique step ID
    pub id: String,
    /// Step description (imperative form: "Add validation to...")
    pub description: String,
    /// Active form for display ("Adding validation to...")
    pub active_description: String,
    /// Detailed instructions for execution
    pub instructions: String,
    /// Estimated files to modify
    pub relevant_files: Vec<String>,
    /// Step dependencies (other step IDs that must complete first)
    pub dependencies: Vec<String>,
    /// Complexity score (0-100)
    pub complexity_score: u8,
    /// Computed complexity level
    pub complexity: StepComplexity,
    /// Assignment decision
    pub assignment: StepAssignment,
    /// Current execution status
    pub status: PlanStepStatus,
    /// Execution output/summary
    pub output: Option<String>,
    /// Execution error if failed
    pub error: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
}

impl PlanStep {
    pub fn new(id: String, description: String) -> Self {
        // Generate active description by converting imperative to present continuous
        let active_description = Self::to_active_form(&description);

        Self {
            id,
            description,
            active_description,
            instructions: String::new(),
            relevant_files: Vec::new(),
            dependencies: Vec::new(),
            complexity_score: 0,
            complexity: StepComplexity::Simple,
            assignment: StepAssignment::Inline,
            status: PlanStepStatus::Pending,
            output: None,
            error: None,
            duration_ms: None,
        }
    }

    /// Convert imperative form to present continuous
    /// "Add validation" -> "Adding validation"
    /// "Create tests" -> "Creating tests"
    fn to_active_form(description: &str) -> String {
        let words: Vec<&str> = description.split_whitespace().collect();
        if words.is_empty() {
            return description.to_string();
        }

        let first_word = words[0];
        let rest = if words.len() > 1 {
            words[1..].join(" ")
        } else {
            String::new()
        };

        // Simple verb transformation
        let active_verb = if first_word.ends_with('e') && !first_word.ends_with("ee") {
            // "Create" -> "Creating", "Update" -> "Updating"
            format!("{}ing", &first_word[..first_word.len() - 1])
        } else if first_word.ends_with('y')
            && first_word.len() > 2
            && !first_word
                .chars()
                .rev()
                .nth(1)
                .map(|c| "aeiou".contains(c))
                .unwrap_or(false)
        {
            // "Modify" -> "Modifying"
            format!("{}ying", &first_word[..first_word.len() - 1])
        } else {
            // "Add" -> "Adding", "Fix" -> "Fixing"
            format!("{}ing", first_word)
        };

        if rest.is_empty() {
            active_verb
        } else {
            format!("{} {}", active_verb, rest)
        }
    }

    pub fn with_instructions(mut self, instructions: String) -> Self {
        self.instructions = instructions;
        self
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.relevant_files = files;
        self
    }

    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }
}

/// Overall plan status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PlanStatus {
    /// LLM is creating the plan
    #[default]
    Planning,
    /// Plan created, awaiting approval (PLAN mode) or ready to execute (BUILD mode)
    Ready,
    /// Waiting for user approval (PLAN mode only)
    AwaitingApproval,
    /// Steps are being executed
    Executing,
    /// All steps completed successfully
    Completed,
    /// One or more steps failed
    Failed,
    /// User cancelled the plan
    Cancelled,
}

impl PlanStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Cancelled
        )
    }
}

/// A complete task plan with steps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique plan ID
    pub id: String,
    /// Original user request
    pub request: String,
    /// Plan title/summary
    pub title: String,
    /// Ordered steps
    pub steps: Vec<PlanStep>,
    /// Overall plan status
    pub status: PlanStatus,
    /// Timestamp when planning started
    pub created_at: DateTime<Utc>,
    /// Timestamp when execution started
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when execution completed
    pub completed_at: Option<DateTime<Utc>>,
}

impl TaskPlan {
    pub fn new(id: String, request: String) -> Self {
        Self {
            id,
            request,
            title: String::new(),
            steps: Vec::new(),
            status: PlanStatus::Planning,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn with_steps(mut self, steps: Vec<PlanStep>) -> Self {
        self.steps = steps;
        self
    }

    /// Get the current step being executed
    pub fn current_step(&self) -> Option<&PlanStep> {
        self.steps
            .iter()
            .find(|s| s.status == PlanStepStatus::InProgress)
    }

    /// Get the index of the current step
    pub fn current_step_index(&self) -> Option<usize> {
        self.steps
            .iter()
            .position(|s| s.status == PlanStepStatus::InProgress)
    }

    /// Check if all dependencies for a step are met
    pub fn dependencies_met(&self, step: &PlanStep) -> bool {
        step.dependencies.iter().all(|dep_id| {
            self.steps
                .iter()
                .find(|s| &s.id == dep_id)
                .map(|s| s.status == PlanStepStatus::Completed)
                .unwrap_or(true) // If dependency not found, assume met
        })
    }

    /// Get count of completed steps
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.status == PlanStepStatus::Completed)
            .count()
    }

    /// Get progress percentage
    pub fn progress_percent(&self) -> f32 {
        if self.steps.is_empty() {
            0.0
        } else {
            (self.completed_count() as f32 / self.steps.len() as f32) * 100.0
        }
    }

    /// Generate a summary of the plan
    pub fn summary(&self) -> String {
        let completed = self.completed_count();
        let total = self.steps.len();
        let failed = self
            .steps
            .iter()
            .filter(|s| s.status == PlanStepStatus::Failed)
            .count();

        if failed > 0 {
            format!(
                "{}: {}/{} steps completed, {} failed",
                self.title, completed, total, failed
            )
        } else {
            format!("{}: {}/{} steps completed", self.title, completed, total)
        }
    }
}

/// Events emitted during plan execution for UI updates
#[derive(Debug, Clone)]
pub enum PlanEvent {
    /// Plan has been created and is ready
    PlanCreated { plan: TaskPlan },
    /// New steps added to an existing plan (for accumulating steps across LLM calls)
    StepsAdded {
        plan_id: String,
        steps: Vec<PlanStep>,
    },
    /// Waiting for user approval (PLAN mode)
    AwaitingApproval { plan_id: String },
    /// User approved the plan
    PlanApproved { plan_id: String },
    /// User rejected the plan
    PlanRejected { plan_id: String },
    /// Step execution started
    StepStarted {
        plan_id: String,
        step_id: String,
        description: String,
    },
    /// Progress update during step execution
    StepProgress {
        plan_id: String,
        step_id: String,
        message: String,
    },
    /// Step completed (success or failure)
    StepCompleted {
        plan_id: String,
        step_id: String,
        success: bool,
        output: Option<String>,
        error: Option<String>,
    },
    /// Plan execution completed
    PlanCompleted {
        plan_id: String,
        success: bool,
        summary: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_active_form() {
        assert_eq!(
            PlanStep::to_active_form("Add validation"),
            "Adding validation"
        );
        assert_eq!(PlanStep::to_active_form("Create tests"), "Creating tests");
        assert_eq!(PlanStep::to_active_form("Update config"), "Updating config");
        assert_eq!(PlanStep::to_active_form("Fix bug"), "Fixing bug");
        assert_eq!(
            PlanStep::to_active_form("Modify schema"),
            "Modifying schema"
        );
    }

    #[test]
    fn test_plan_step_status_icon() {
        assert_eq!(PlanStepStatus::Pending.icon(), "◯");
        assert_eq!(PlanStepStatus::InProgress.icon(), "◐");
        assert_eq!(PlanStepStatus::Completed.icon(), "✓");
        assert_eq!(PlanStepStatus::Failed.icon(), "✗");
    }

    #[test]
    fn test_plan_progress() {
        let mut plan = TaskPlan::new("test".to_string(), "Test request".to_string());
        plan.steps = vec![
            PlanStep::new("1".to_string(), "Step 1".to_string()),
            PlanStep::new("2".to_string(), "Step 2".to_string()),
            PlanStep::new("3".to_string(), "Step 3".to_string()),
        ];

        assert_eq!(plan.progress_percent(), 0.0);

        plan.steps[0].status = PlanStepStatus::Completed;
        assert!((plan.progress_percent() - 33.33).abs() < 1.0);

        plan.steps[1].status = PlanStepStatus::Completed;
        assert!((plan.progress_percent() - 66.66).abs() < 1.0);
    }
}
