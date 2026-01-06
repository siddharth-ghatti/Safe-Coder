//! Example configurations for self-orchestration with multiple safe-coder instances
//!
//! This shows how to configure safe-coder to orchestrate across multiple instances
//! of itself, enabling sophisticated parallel development workflows.

use safe_coder::orchestrator::{OrchestratorConfig, WorkerStrategy, ThrottleLimits, WorkerKind};
use safe_coder::approval::ExecutionMode;

/// Example 1: Pure Self-Orchestration
/// 
/// This configuration uses only safe-coder instances as workers,
/// creating a pure self-orchestrating system.
pub fn pure_self_orchestration_config() -> OrchestratorConfig {
    OrchestratorConfig {
        claude_cli_path: None, // Disable external CLIs
        gemini_cli_path: None,
        safe_coder_cli_path: Some("safe-coder".to_string()),
        gh_cli_path: None,
        max_workers: 4, // Allow 4 concurrent safe-coder instances
        default_worker: WorkerKind::SafeCoder,
        worker_strategy: WorkerStrategy::LoadBalanced, // Distribute evenly
        enabled_workers: vec![WorkerKind::SafeCoder], // Only safe-coder
        use_worktrees: true, // Essential for isolation
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 0, // Disabled
            gemini_max_concurrent: 0, // Disabled
            safe_coder_max_concurrent: 4, // Up to 4 safe-coder instances
            copilot_max_concurrent: 0, // Disabled
            start_delay_ms: 500, // Stagger starts to avoid resource conflicts
        },
        execution_mode: ExecutionMode::Act, // Auto-execute for smooth orchestration
    }
}

/// Example 2: Mixed Self-Orchestration
/// 
/// This configuration combines safe-coder with other CLIs,
/// using safe-coder as the primary orchestrator but with backup options.
pub fn mixed_self_orchestration_config() -> OrchestratorConfig {
    OrchestratorConfig {
        claude_cli_path: Some("claude".to_string()),
        gemini_cli_path: Some("gemini".to_string()),
        safe_coder_cli_path: Some("safe-coder".to_string()),
        gh_cli_path: Some("gh".to_string()),
        max_workers: 6, // Higher concurrency with multiple tools
        default_worker: WorkerKind::SafeCoder, // Prefer self-orchestration
        worker_strategy: WorkerStrategy::TaskBased, // Smart assignment
        enabled_workers: vec![
            WorkerKind::SafeCoder,   // Primary
            WorkerKind::ClaudeCode,  // For complex tasks
            WorkerKind::GitHubCopilot, // For simple tasks
        ],
        use_worktrees: true,
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 2,
            gemini_max_concurrent: 1,
            safe_coder_max_concurrent: 3, // Most capacity for self-orchestration
            copilot_max_concurrent: 2,
            start_delay_ms: 200,
        },
        execution_mode: ExecutionMode::Plan, // Show plan before executing
    }
}

/// Example 3: Hierarchical Self-Orchestration
/// 
/// This configuration is designed for complex projects where safe-coder
/// can spawn sub-orchestrators that handle specific domains.
pub fn hierarchical_self_orchestration_config() -> OrchestratorConfig {
    OrchestratorConfig {
        claude_cli_path: None,
        gemini_cli_path: None,
        safe_coder_cli_path: Some("safe-coder".to_string()),
        gh_cli_path: None,
        max_workers: 8, // High concurrency for complex workflows
        default_worker: WorkerKind::SafeCoder,
        worker_strategy: WorkerStrategy::LoadBalanced,
        enabled_workers: vec![WorkerKind::SafeCoder],
        use_worktrees: true,
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 0,
            gemini_max_concurrent: 0,
            safe_coder_max_concurrent: 8, // Maximum self-orchestration
            copilot_max_concurrent: 0,
            start_delay_ms: 100, // Faster starts for high throughput
        },
        execution_mode: ExecutionMode::Act,
    }
}

/// Example usage patterns for self-orchestration
pub mod usage_examples {
    use super::*;

    /// Example of a request that would benefit from self-orchestration
    pub const COMPLEX_REFACTORING_REQUEST: &str = r#"
I need to refactor our entire authentication system to support OAuth2. This involves:

1. Update the user model to store OAuth tokens
2. Create new OAuth2 service modules  
3. Modify all existing auth middleware
4. Update the API endpoints to use new auth
5. Write comprehensive tests for all changes
6. Update documentation

Each of these can be worked on in parallel since they touch different parts of the codebase.
"#;

    /// Example of how safe-coder would plan this for self-orchestration
    pub const EXPECTED_SELF_ORCHESTRATION_PLAN: &str = r#"
🎯 ORCHESTRATION PLAN
══════════════════════════════════════════════════════════════

📝 Request: Refactor authentication system to support OAuth2

📋 Summary: Large-scale authentication refactoring requiring parallel development across multiple modules

🔧 Tasks (6):
───────────────────────────────────────────────────────────────

  1. 🛡️ SafeCoder
     📌 Update user model for OAuth token storage
     📁 Files: src/auth/models.rs, migrations/
     💬 Instructions: Add OAuth2 token fields to User model...

  2. 🛡️ SafeCoder
     📌 Implement OAuth2 service modules
     📁 Files: src/auth/oauth2/
     💬 Instructions: Create OAuth2 client, token management...

  3. 🛡️ SafeCoder
     📌 Update authentication middleware
     📁 Files: src/auth/middleware.rs
     🔗 Depends on: task-1
     💬 Instructions: Modify middleware to handle OAuth tokens...

  4. 🛡️ SafeCoder
     📌 Update API endpoints for new auth
     📁 Files: src/api/
     🔗 Depends on: task-2, task-3
     💬 Instructions: Update all auth-related endpoints...

  5. 🛡️ SafeCoder
     📌 Write comprehensive tests
     📁 Files: tests/auth/
     💬 Instructions: Create unit and integration tests...

  6. 🛡️ SafeCoder
     📌 Update documentation
     📁 Files: docs/
     💬 Instructions: Update auth documentation...

───────────────────────────────────────────────────────────────

⚙️  Execution Configuration:
   • Max concurrent workers: 4
   • Safe-coder max concurrent: 4
   • Worker start delay: 500ms
   • Using worktrees: true
"#;
}