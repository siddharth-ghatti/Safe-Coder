//! Unified Planning Types
//!
//! Core types for the unified planning system that work across all execution modes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Execution mode determines how steps are executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    /// Direct: Execute inline in Safe-Coder session
    /// - No parallelism, sequential tool calls
    /// - Best for simple, single-file tasks
    #[default]
    Direct,

    /// Subagent: Spawn internal specialized agents
    /// - Parallel within same process
    /// - Share context, can coordinate
    /// - Best for medium complexity with related subtasks
    Subagent,

    /// Orchestration: Delegate to external CLIs in git worktrees
    /// - Full parallelism with process isolation
    /// - Each worker gets own workspace
    /// - Best for large tasks with independent parts
    Orchestration,
}

impl fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionMode::Direct => write!(f, "Direct"),
            ExecutionMode::Subagent => write!(f, "Subagent"),
            ExecutionMode::Orchestration => write!(f, "Orchestration"),
        }
    }
}

/// Who should execute this step
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StepExecutor {
    /// Execute inline in current session
    Inline,
    /// Delegate to a subagent
    Subagent {
        #[serde(default)]
        kind: SubagentKind,
    },
    /// Delegate to external CLI worker
    Worker {
        #[serde(default)]
        kind: WorkerKind,
    },
}

impl Default for StepExecutor {
    fn default() -> Self {
        StepExecutor::Inline
    }
}

/// Subagent kinds for internal specialized agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SubagentKind {
    /// Read-only code analysis
    #[default]
    CodeAnalyzer,
    /// Create and run tests
    Tester,
    /// Refactor existing code
    Refactorer,
    /// Generate documentation
    Documenter,
    /// User-defined custom agent
    Custom,
}

impl fmt::Display for SubagentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubagentKind::CodeAnalyzer => write!(f, "analyzer"),
            SubagentKind::Tester => write!(f, "tester"),
            SubagentKind::Refactorer => write!(f, "refactorer"),
            SubagentKind::Documenter => write!(f, "documenter"),
            SubagentKind::Custom => write!(f, "custom"),
        }
    }
}

/// External CLI worker kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkerKind {
    /// Claude Code CLI
    #[default]
    ClaudeCode,
    /// Gemini CLI
    GeminiCli,
    /// Safe-Coder itself (recursive)
    SafeCoder,
    /// GitHub Copilot via gh CLI
    GitHubCopilot,
}

impl fmt::Display for WorkerKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkerKind::ClaudeCode => write!(f, "claude"),
            WorkerKind::GeminiCli => write!(f, "gemini"),
            WorkerKind::SafeCoder => write!(f, "safe-coder"),
            WorkerKind::GitHubCopilot => write!(f, "copilot"),
        }
    }
}

/// Step execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl StepStatus {
    /// Check if the step is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepStatus::Completed | StepStatus::Failed | StepStatus::Skipped
        )
    }

    /// Get icon for display
    pub fn icon(&self) -> &'static str {
        match self {
            StepStatus::Pending => "◯",
            StepStatus::InProgress => "◐",
            StepStatus::Completed => "✓",
            StepStatus::Failed => "✗",
            StepStatus::Skipped => "−",
        }
    }
}

/// Plan status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    #[default]
    Planning,
    Ready,
    AwaitingApproval,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl PlanStatus {
    /// Check if the plan is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PlanStatus::Completed | PlanStatus::Failed | PlanStatus::Cancelled
        )
    }
}

/// Result of executing a step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    /// Whether the step succeeded
    pub success: bool,
    /// Output from the step execution
    pub output: String,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Files that were modified
    pub files_modified: Vec<String>,
}

impl Default for StepResult {
    fn default() -> Self {
        Self {
            success: false,
            output: String::new(),
            error: None,
            duration_ms: 0,
            files_modified: Vec::new(),
        }
    }
}

/// A single step in the unified plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStep {
    /// Unique step ID
    pub id: String,
    /// Imperative description ("Add validation to form")
    pub description: String,
    /// Present continuous form ("Adding validation to form")
    pub active_description: String,
    /// Detailed instructions for execution
    pub instructions: String,
    /// Files this step will touch
    pub relevant_files: Vec<String>,
    /// Complexity score (0-100)
    pub complexity_score: u8,
    /// Suggested executor for this step
    pub suggested_executor: StepExecutor,
    /// Current status
    pub status: StepStatus,
    /// Execution result (populated after execution)
    pub result: Option<StepResult>,
}

impl UnifiedStep {
    /// Create a new step with basic info
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        let desc = description.into();
        let active = to_active_form(&desc);
        Self {
            id: id.into(),
            description: desc,
            active_description: active,
            instructions: String::new(),
            relevant_files: Vec::new(),
            complexity_score: 0,
            suggested_executor: StepExecutor::Inline,
            status: StepStatus::Pending,
            result: None,
        }
    }

    /// Add instructions
    pub fn with_instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = instructions.into();
        self
    }

    /// Add relevant files
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.relevant_files = files;
        self
    }

    /// Set complexity score
    pub fn with_complexity(mut self, score: u8) -> Self {
        self.complexity_score = score;
        self
    }

    /// Set suggested executor
    pub fn with_executor(mut self, executor: StepExecutor) -> Self {
        self.suggested_executor = executor;
        self
    }

    /// Check if step is completed successfully
    pub fn is_completed(&self) -> bool {
        matches!(self.status, StepStatus::Completed)
    }

    /// Check if step failed
    pub fn is_failed(&self) -> bool {
        matches!(self.status, StepStatus::Failed)
    }
}

/// A group of steps that can execute in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepGroup {
    /// Unique group identifier
    pub id: String,
    /// Steps in this group (can run in parallel)
    pub steps: Vec<UnifiedStep>,
    /// Group IDs that must complete before this group starts
    pub depends_on: Vec<String>,
}

impl StepGroup {
    /// Create a new step group
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            steps: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Add a step to the group
    pub fn add_step(mut self, step: UnifiedStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Add multiple steps
    pub fn with_steps(mut self, steps: Vec<UnifiedStep>) -> Self {
        self.steps = steps;
        self
    }

    /// Add dependency on another group
    pub fn depends_on(mut self, group_id: impl Into<String>) -> Self {
        self.depends_on.push(group_id.into());
        self
    }

    /// Check if this group can run in parallel (has multiple steps)
    pub fn is_parallel(&self) -> bool {
        self.steps.len() > 1
    }

    /// Get total step count
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// Check if all steps in group are completed
    pub fn is_completed(&self) -> bool {
        self.steps.iter().all(|s| s.status.is_terminal())
    }

    /// Check if all steps succeeded
    pub fn is_successful(&self) -> bool {
        self.steps.iter().all(|s| s.is_completed())
    }

    /// Get count of completed steps
    pub fn completed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Completed))
            .count()
    }

    /// Get count of failed steps
    pub fn failed_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Failed))
            .count()
    }
}

/// The unified plan structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedPlan {
    /// Unique plan ID
    pub id: String,
    /// Original user request
    pub request: String,
    /// Plan title/summary
    pub title: String,
    /// Execution mode for this plan
    pub execution_mode: ExecutionMode,
    /// Ordered groups of steps (groups execute sequentially, steps within group can be parallel)
    pub groups: Vec<StepGroup>,
    /// Overall plan status
    pub status: PlanStatus,
    /// When planning started
    pub created_at: DateTime<Utc>,
    /// When execution started
    pub started_at: Option<DateTime<Utc>>,
    /// When execution completed
    pub completed_at: Option<DateTime<Utc>>,
}

impl UnifiedPlan {
    /// Create a new plan
    pub fn new(id: impl Into<String>, request: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            request: request.into(),
            title: String::new(),
            execution_mode: ExecutionMode::Direct,
            groups: Vec::new(),
            status: PlanStatus::Planning,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the execution mode
    pub fn with_mode(mut self, mode: ExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    /// Add a step group
    pub fn add_group(mut self, group: StepGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Set groups
    pub fn with_groups(mut self, groups: Vec<StepGroup>) -> Self {
        self.groups = groups;
        self
    }

    /// Get all steps flattened
    pub fn all_steps(&self) -> Vec<&UnifiedStep> {
        self.groups.iter().flat_map(|g| &g.steps).collect()
    }

    /// Get all steps mutably
    pub fn all_steps_mut(&mut self) -> Vec<&mut UnifiedStep> {
        self.groups.iter_mut().flat_map(|g| &mut g.steps).collect()
    }

    /// Get total step count
    pub fn total_steps(&self) -> usize {
        self.groups.iter().map(|g| g.steps.len()).sum()
    }

    /// Get count of parallel groups
    pub fn parallel_group_count(&self) -> usize {
        self.groups.iter().filter(|g| g.is_parallel()).count()
    }

    /// Get count of completed steps
    pub fn completed_steps(&self) -> usize {
        self.all_steps()
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Completed))
            .count()
    }

    /// Get count of failed steps
    pub fn failed_steps(&self) -> usize {
        self.all_steps()
            .iter()
            .filter(|s| matches!(s.status, StepStatus::Failed))
            .count()
    }

    /// Calculate progress percentage
    pub fn progress(&self) -> f32 {
        let total = self.total_steps();
        if total == 0 {
            return 0.0;
        }
        let completed = self.completed_steps();
        (completed as f32 / total as f32) * 100.0
    }

    /// Check if all dependencies are met for a group
    pub fn dependencies_met(&self, group: &StepGroup) -> bool {
        group.depends_on.iter().all(|dep_id| {
            self.groups
                .iter()
                .find(|g| &g.id == dep_id)
                .map(|g| g.is_completed())
                .unwrap_or(true) // If dependency not found, assume met
        })
    }

    /// Get a summary string
    pub fn summary(&self) -> String {
        let total = self.total_steps();
        let completed = self.completed_steps();
        let failed = self.failed_steps();
        let parallel = self.parallel_group_count();

        format!(
            "{}: {}/{} steps ({} failed), {} parallel groups, mode: {}",
            self.title, completed, total, failed, parallel, self.execution_mode
        )
    }

    /// Mark plan as ready
    pub fn mark_ready(&mut self) {
        self.status = PlanStatus::Ready;
    }

    /// Mark plan as executing
    pub fn mark_executing(&mut self) {
        self.status = PlanStatus::Executing;
        self.started_at = Some(Utc::now());
    }

    /// Mark plan as completed
    pub fn mark_completed(&mut self) {
        let has_failures = self.failed_steps() > 0;
        self.status = if has_failures {
            PlanStatus::Failed
        } else {
            PlanStatus::Completed
        };
        self.completed_at = Some(Utc::now());
    }

    /// Update status of a specific step by ID
    pub fn update_step_status(&mut self, step_id: &str, status: StepStatus) {
        for group in &mut self.groups {
            for step in &mut group.steps {
                if step.id == step_id {
                    step.status = status;
                    return;
                }
            }
        }
    }

    /// Find a step by ID
    pub fn find_step(&self, step_id: &str) -> Option<&UnifiedStep> {
        for group in &self.groups {
            for step in &group.steps {
                if step.id == step_id {
                    return Some(step);
                }
            }
        }
        None
    }

    /// Find a step mutably by ID
    pub fn find_step_mut(&mut self, step_id: &str) -> Option<&mut UnifiedStep> {
        for group in &mut self.groups {
            for step in &mut group.steps {
                if step.id == step_id {
                    return Some(step);
                }
            }
        }
        None
    }

    /// Convert to legacy TaskPlan for TUI compatibility
    pub fn to_legacy_plan(&self) -> crate::planning::TaskPlan {
        use crate::planning::{PlanStatus as LegacyStatus, PlanStep, PlanStepStatus, TaskPlan};

        let legacy_status = match self.status {
            PlanStatus::Planning => LegacyStatus::Planning,
            PlanStatus::Ready => LegacyStatus::Ready,
            PlanStatus::AwaitingApproval => LegacyStatus::AwaitingApproval,
            PlanStatus::Executing => LegacyStatus::Executing,
            PlanStatus::Completed => LegacyStatus::Completed,
            PlanStatus::Failed => LegacyStatus::Failed,
            PlanStatus::Cancelled => LegacyStatus::Cancelled,
        };

        let mut legacy_plan = TaskPlan::new(self.id.clone(), self.request.clone())
            .with_title(self.title.clone());
        legacy_plan.status = legacy_status;

        // Convert steps from groups
        let mut legacy_steps: Vec<PlanStep> = Vec::new();
        for group in &self.groups {
            for step in &group.steps {
                let legacy_step_status = match step.status {
                    StepStatus::Pending => PlanStepStatus::Pending,
                    StepStatus::InProgress => PlanStepStatus::InProgress,
                    StepStatus::Completed => PlanStepStatus::Completed,
                    StepStatus::Failed => PlanStepStatus::Failed,
                    StepStatus::Skipped => PlanStepStatus::Skipped,
                };

                let mut legacy_step = PlanStep::new(step.id.clone(), step.description.clone())
                    .with_instructions(step.instructions.clone())
                    .with_files(step.relevant_files.clone());
                legacy_step.active_description = step.active_description.clone();
                legacy_step.status = legacy_step_status;

                // Add dependencies from group
                if !group.depends_on.is_empty() {
                    legacy_step.dependencies = group.depends_on.clone();
                }

                legacy_steps.push(legacy_step);
            }
        }

        legacy_plan.steps = legacy_steps;
        legacy_plan
    }
}

/// Convert imperative description to present continuous form
/// "Add validation" -> "Adding validation"
/// "Create new file" -> "Creating new file"
/// "Fix the bug" -> "Fixing the bug"
pub fn to_active_form(description: &str) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Split into first word and rest
    let mut words = trimmed.splitn(2, ' ');
    let first = words.next().unwrap_or("");
    let rest = words.next().unwrap_or("");

    // Convert first word to -ing form
    let first_lower = first.to_lowercase();
    let active_first = if first_lower.ends_with("e") && !first_lower.ends_with("ee") {
        // remove -> removing, create -> creating
        format!("{}ing", &first[..first.len() - 1])
    } else if first_lower.ends_with("ie") {
        // die -> dying (rare in code context)
        format!("{}ying", &first[..first.len() - 2])
    } else if should_double_consonant(&first_lower) {
        // run -> running, set -> setting
        format!("{}{}ing", first, first.chars().last().unwrap_or(' '))
    } else {
        format!("{}ing", first)
    };

    // Preserve original capitalization
    let active_first = if first
        .chars()
        .next()
        .map(|c| c.is_uppercase())
        .unwrap_or(false)
    {
        let mut chars = active_first.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => active_first,
        }
    } else {
        active_first
    };

    if rest.is_empty() {
        active_first
    } else {
        format!("{} {}", active_first, rest)
    }
}

/// Check if we should double the final consonant before adding -ing
fn should_double_consonant(word: &str) -> bool {
    if word.len() < 2 {
        return false;
    }

    let chars: Vec<char> = word.chars().collect();
    let last = chars[chars.len() - 1];
    let second_last = chars[chars.len() - 2];

    // Check if last is consonant and second-last is vowel
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let is_last_consonant = last.is_alphabetic() && !vowels.contains(&last);
    let is_second_vowel = vowels.contains(&second_last);

    // Special cases - don't double w, x, y
    if ['w', 'x', 'y'].contains(&last) {
        return false;
    }

    is_last_consonant && is_second_vowel && word.len() <= 4
}

/// Events emitted during plan execution
#[derive(Debug, Clone)]
pub enum PlanEvent {
    /// Plan was created - includes full plan for UI display
    PlanCreated {
        plan_id: String,
        title: String,
        total_steps: usize,
        execution_mode: ExecutionMode,
        /// Full plan for UI display
        plan: UnifiedPlan,
    },
    /// Plan is awaiting user approval
    PlanAwaitingApproval { plan_id: String },
    /// Plan was approved
    PlanApproved { plan_id: String },
    /// Plan was rejected
    PlanRejected { plan_id: String, reason: String },
    /// Plan execution started
    PlanStarted { plan_id: String },
    /// A step group started executing
    GroupStarted {
        plan_id: String,
        group_id: String,
        parallel_count: usize,
    },
    /// A step started executing
    StepStarted {
        plan_id: String,
        group_id: String,
        step_id: String,
        description: String,
    },
    /// Progress update for a step
    StepProgress {
        plan_id: String,
        step_id: String,
        message: String,
    },
    /// A step completed
    StepCompleted {
        plan_id: String,
        step_id: String,
        success: bool,
        duration_ms: u64,
    },
    /// A file was modified
    FileModified {
        plan_id: String,
        step_id: String,
        path: String,
        old_content: String,
        new_content: String,
    },
    /// A step group completed
    GroupCompleted {
        plan_id: String,
        group_id: String,
        success: bool,
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
        assert_eq!(to_active_form("Add validation"), "Adding validation");
        assert_eq!(to_active_form("Create new file"), "Creating new file");
        assert_eq!(to_active_form("Fix the bug"), "Fixing the bug");
        assert_eq!(to_active_form("Run tests"), "Running tests");
        assert_eq!(to_active_form("Set config"), "Setting config");
        assert_eq!(to_active_form("Update"), "Updating");
    }

    #[test]
    fn test_step_status_terminal() {
        assert!(!StepStatus::Pending.is_terminal());
        assert!(!StepStatus::InProgress.is_terminal());
        assert!(StepStatus::Completed.is_terminal());
        assert!(StepStatus::Failed.is_terminal());
        assert!(StepStatus::Skipped.is_terminal());
    }

    #[test]
    fn test_step_group_parallel() {
        let group = StepGroup::new("g1")
            .add_step(UnifiedStep::new("s1", "Step 1"))
            .add_step(UnifiedStep::new("s2", "Step 2"));

        assert!(group.is_parallel());
        assert_eq!(group.step_count(), 2);
    }

    #[test]
    fn test_unified_plan_progress() {
        let mut plan = UnifiedPlan::new("plan-1", "Test request").with_title("Test Plan");

        let mut step1 = UnifiedStep::new("s1", "Step 1");
        step1.status = StepStatus::Completed;
        let step2 = UnifiedStep::new("s2", "Step 2");

        plan = plan.add_group(StepGroup::new("g1").with_steps(vec![step1, step2]));

        assert_eq!(plan.total_steps(), 2);
        assert_eq!(plan.completed_steps(), 1);
        assert_eq!(plan.progress(), 50.0);
    }

    #[test]
    fn test_execution_mode_display() {
        assert_eq!(format!("{}", ExecutionMode::Direct), "Direct");
        assert_eq!(format!("{}", ExecutionMode::Subagent), "Subagent");
        assert_eq!(format!("{}", ExecutionMode::Orchestration), "Orchestration");
    }
}
