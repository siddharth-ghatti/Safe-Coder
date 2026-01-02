//! Complexity scoring algorithm for plan steps
//!
//! Determines how complex a step is. Subagent support is disabled
//! for now - all steps execute inline.

use super::types::{PlanStep, StepAssignment, StepComplexity, TaskPlan};

/// Calculate complexity score for a step (0-100)
pub fn calculate_complexity(step: &PlanStep, _plan: &TaskPlan) -> u8 {
    let mut score: u32 = 0;

    // Factor 1: File count (0-20 points, 4 pts per file, max 5 files)
    score += (step.relevant_files.len().min(5) * 4) as u32;

    // Factor 2: Operation type keywords (0-30 points)
    let desc_lower = step.description.to_lowercase();
    if contains_any(
        &desc_lower,
        &["refactor", "rewrite", "redesign", "migrate", "restructure"],
    ) {
        score += 30;
    } else if contains_any(
        &desc_lower,
        &[
            "implement",
            "create",
            "add feature",
            "build",
            "develop",
            "integrate",
        ],
    ) {
        score += 20;
    } else if contains_any(
        &desc_lower,
        &["fix", "update", "modify", "test", "change", "adjust"],
    ) {
        score += 15;
    } else if contains_any(
        &desc_lower,
        &[
            "read", "analyze", "check", "document", "review", "explore", "find",
        ],
    ) {
        score += 5;
    }

    // Factor 3: Description length as proxy for scope (0-15 points)
    score += (step.description.len().min(300) / 20) as u32;

    // Factor 4: Dependency count (0-15 points, 5 pts per dependency, max 3)
    score += (step.dependencies.len().min(3) * 5) as u32;

    // Factor 5: Instructions complexity (0-20 points)
    score += (step.instructions.len().min(1000) / 50) as u32;

    score.min(100) as u8
}

/// Convert complexity score to complexity level
pub fn complexity_from_score(score: u8) -> StepComplexity {
    match score {
        0..=30 => StepComplexity::Simple,
        31..=60 => StepComplexity::Medium,
        _ => StepComplexity::Complex,
    }
}

/// Determine subagent assignment based on complexity and step content
/// NOTE: Subagents are currently disabled - all steps execute inline
pub fn assign_step(_step: &PlanStep) -> StepAssignment {
    // All steps execute inline for now while we perfect the planning system
    // Subagent support will be re-enabled later
    StepAssignment::Inline
}

/// Score and assign all steps in a plan
pub fn score_and_assign_plan(plan: &mut TaskPlan) {
    // First pass: calculate complexity scores
    let plan_clone = plan.clone();
    for step in &mut plan.steps {
        step.complexity_score = calculate_complexity(step, &plan_clone);
        step.complexity = complexity_from_score(step.complexity_score);
    }

    // Second pass: assign execution method
    for step in &mut plan.steps {
        step.assignment = assign_step(step);
    }
}

/// Check if text contains any of the keywords
fn contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|k| text.contains(k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_scoring() {
        let plan = TaskPlan::new("test".to_string(), "Test".to_string());

        // Simple step
        let simple_step = PlanStep::new("1".to_string(), "Fix typo in readme".to_string());
        let score = calculate_complexity(&simple_step, &plan);
        assert!(score <= 30, "Simple step should have low score: {}", score);

        // Complex step with files
        let mut complex_step = PlanStep::new(
            "2".to_string(),
            "Refactor authentication module across the entire codebase with comprehensive restructuring".to_string(),
        );
        complex_step.relevant_files = vec![
            "src/auth/mod.rs".to_string(),
            "src/auth/login.rs".to_string(),
            "src/auth/session.rs".to_string(),
            "src/auth/token.rs".to_string(),
            "src/auth/middleware.rs".to_string(),
        ];
        complex_step.dependencies = vec!["step-1".to_string()];
        complex_step.instructions = "This is a detailed refactoring task that involves multiple files and significant architectural changes to improve the authentication flow. You will need to update the login logic, session management, token handling, and middleware. Make sure to preserve backwards compatibility while introducing the new patterns. Consider edge cases and error handling. Update all related tests and documentation.".to_string();
        let score = calculate_complexity(&complex_step, &plan);
        assert!(score > 60, "Complex step should have high score: {}", score);
    }

    #[test]
    fn test_complexity_from_score() {
        assert_eq!(complexity_from_score(0), StepComplexity::Simple);
        assert_eq!(complexity_from_score(30), StepComplexity::Simple);
        assert_eq!(complexity_from_score(31), StepComplexity::Medium);
        assert_eq!(complexity_from_score(60), StepComplexity::Medium);
        assert_eq!(complexity_from_score(61), StepComplexity::Complex);
        assert_eq!(complexity_from_score(100), StepComplexity::Complex);
    }

    #[test]
    fn test_subagent_assignment() {
        // All steps are inline now (subagents disabled)
        let mut test_step =
            PlanStep::new("1".to_string(), "Write unit tests for parser".to_string());
        test_step.complexity = StepComplexity::Medium;
        let assignment = assign_step(&test_step);
        assert!(matches!(assignment, StepAssignment::Inline));

        let mut complex_step =
            PlanStep::new("2".to_string(), "Refactor database module".to_string());
        complex_step.complexity = StepComplexity::Complex;
        let assignment = assign_step(&complex_step);
        assert!(matches!(assignment, StepAssignment::Inline));

        let mut simple_step = PlanStep::new("3".to_string(), "Update version number".to_string());
        simple_step.complexity = StepComplexity::Simple;
        let assignment = assign_step(&simple_step);
        assert!(matches!(assignment, StepAssignment::Inline));
    }
}
