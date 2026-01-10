//! Enhanced self-orchestration capabilities for safe-coder
//!
//! This module extends the orchestration system with patterns specifically 
//! designed for safe-coder orchestrating multiple instances of itself.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::process::Command;

use crate::orchestrator::{OrchestratorConfig, WorkerKind};

/// Self-orchestration manager that handles safe-coder specific orchestration patterns
pub struct SelfOrchestrationManager {
    /// Base configuration for orchestration
    base_config: OrchestratorConfig,
    /// Active self-orchestrated instances
    active_instances: HashMap<String, SelfOrchestratedInstance>,
    /// Self-orchestration strategies
    strategies: SelfOrchestrationStrategies,
}

/// Represents an active self-orchestrated safe-coder instance
#[derive(Debug, Clone)]
pub struct SelfOrchestratedInstance {
    /// Unique identifier for this instance
    pub instance_id: String,
    /// Task being handled by this instance
    pub task_id: String,
    /// Workspace path for this instance
    pub workspace_path: PathBuf,
    /// Specialized role of this instance
    pub role: InstanceRole,
    /// Process ID of the running instance
    pub process_id: Option<u32>,
    /// Configuration passed to this instance
    pub instance_config: InstanceConfig,
}

/// Specialized roles that self-orchestrated instances can take
#[derive(Debug, Clone, PartialEq)]
pub enum InstanceRole {
    /// General-purpose worker
    GeneralWorker,
    /// Specialized for testing tasks
    TestSpecialist,
    /// Specialized for documentation tasks
    DocumentationSpecialist,
    /// Specialized for refactoring tasks
    RefactoringSpecialist,
    /// Specialized for code generation
    CodeGenerator,
    /// Specialized for bug fixing
    BugFixer,
    /// Specialized for performance optimization
    PerformanceOptimizer,
    /// Meta-orchestrator that manages other instances
    MetaOrchestrator,
}

/// Configuration for a specific safe-coder instance
#[derive(Debug, Clone)]
pub struct InstanceConfig {
    /// Role-specific settings
    pub role_config: RoleConfig,
    /// LLM provider preferences for this instance
    pub llm_preferences: LlmPreferences,
    /// Tool restrictions for this instance
    pub allowed_tools: Vec<String>,
    /// Maximum execution time for this instance
    pub max_execution_time_seconds: u64,
    /// Memory/resource limits
    pub resource_limits: ResourceLimits,
}

/// Role-specific configuration
#[derive(Debug, Clone)]
pub enum RoleConfig {
    GeneralWorker {
        max_file_changes: usize,
    },
    TestSpecialist {
        test_frameworks: Vec<String>,
        coverage_threshold: f64,
    },
    DocumentationSpecialist {
        doc_formats: Vec<String>,
        update_existing: bool,
    },
    RefactoringSpecialist {
        max_complexity_score: f64,
        preserve_api: bool,
    },
    CodeGenerator {
        templates_path: Option<PathBuf>,
        style_guides: Vec<String>,
    },
    BugFixer {
        max_risk_level: RiskLevel,
        require_tests: bool,
    },
    PerformanceOptimizer {
        target_metrics: Vec<String>,
        profiling_enabled: bool,
    },
    MetaOrchestrator {
        max_sub_instances: usize,
        delegation_strategy: DelegationStrategy,
    },
}

/// Risk levels for bug fixing
#[derive(Debug, Clone, PartialEq)]
pub enum RiskLevel {
    Low,    // Safe changes only
    Medium, // Some risk acceptable
    High,   // Any changes needed
}

/// Strategy for delegating work in meta-orchestration
#[derive(Debug, Clone)]
pub enum DelegationStrategy {
    ByComplexity,   // Delegate based on task complexity
    ByDomain,       // Delegate based on code domain (frontend, backend, etc.)
    ByFileType,     // Delegate based on file types involved
    LoadBalanced,   // Even distribution
}

/// LLM provider preferences for an instance
#[derive(Debug, Clone)]
pub struct LlmPreferences {
    /// Preferred provider (claude, openai, etc.)
    pub preferred_provider: Option<String>,
    /// Model preferences in order
    pub model_preferences: Vec<String>,
    /// Temperature setting for this role
    pub temperature: Option<f32>,
    /// Max tokens for responses
    pub max_tokens: Option<u32>,
}

/// Resource limits for an instance
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory usage (MB)
    pub max_memory_mb: Option<u64>,
    /// Maximum CPU percentage
    pub max_cpu_percent: Option<f32>,
    /// Maximum disk usage (MB)
    pub max_disk_mb: Option<u64>,
}

/// Self-orchestration strategies
#[derive(Debug, Clone)]
pub struct SelfOrchestrationStrategies {
    /// Auto-detect optimal instance roles based on task content
    pub auto_role_detection: bool,
    /// Enable hierarchical orchestration (instances can spawn sub-instances)
    pub hierarchical_orchestration: bool,
    /// Dynamic load balancing between instances
    pub dynamic_load_balancing: bool,
    /// Cross-instance communication for coordination
    pub inter_instance_communication: bool,
}

impl SelfOrchestrationManager {
    /// Create a new self-orchestration manager
    pub fn new(base_config: OrchestratorConfig) -> Self {
        Self {
            base_config,
            active_instances: HashMap::new(),
            strategies: SelfOrchestrationStrategies::default(),
        }
    }

    /// Configure self-orchestration strategies
    pub fn with_strategies(mut self, strategies: SelfOrchestrationStrategies) -> Self {
        self.strategies = strategies;
        self
    }

    /// Spawn a specialized safe-coder instance for a specific task
    pub async fn spawn_specialized_instance(
        &mut self,
        task_id: String,
        task_description: &str,
        relevant_files: &[String],
        workspace_path: PathBuf,
    ) -> Result<String> {
        // Auto-detect optimal role if enabled
        let role = if self.strategies.auto_role_detection {
            self.detect_optimal_role(task_description, relevant_files)
        } else {
            InstanceRole::GeneralWorker
        };

        let instance_id = format!("{}_{}", task_id, uuid::Uuid::new_v4().simple());
        let instance_config = self.create_instance_config(&role);

        // Build the safe-coder command with role-specific arguments
        let mut cmd = self.build_instance_command(&instance_config, &workspace_path)?;
        
        // Add the task instructions
        cmd.arg("act").arg(task_description);

        // Spawn the process
        let child = cmd.spawn()?;
        let process_id = child.id();

        let instance = SelfOrchestratedInstance {
            instance_id: instance_id.clone(),
            task_id,
            workspace_path,
            role,
            process_id,
            instance_config,
        };

        self.active_instances.insert(instance_id.clone(), instance);

        Ok(instance_id)
    }

    /// Auto-detect the optimal role for a task
    fn detect_optimal_role(&self, task_description: &str, relevant_files: &[String]) -> InstanceRole {
        let task_lower = task_description.to_lowercase();
        
        // Check for testing keywords
        if task_lower.contains("test") || task_lower.contains("spec") || 
           relevant_files.iter().any(|f| f.contains("test") || f.contains("spec")) {
            return InstanceRole::TestSpecialist;
        }

        // Check for documentation keywords
        if task_lower.contains("document") || task_lower.contains("readme") ||
           relevant_files.iter().any(|f| f.contains(".md") || f.contains("doc")) {
            return InstanceRole::DocumentationSpecialist;
        }

        // Check for refactoring keywords
        if task_lower.contains("refactor") || task_lower.contains("reorganize") ||
           task_lower.contains("restructure") {
            return InstanceRole::RefactoringSpecialist;
        }

        // Check for bug fixing keywords
        if task_lower.contains("fix") || task_lower.contains("bug") ||
           task_lower.contains("error") || task_lower.contains("issue") {
            return InstanceRole::BugFixer;
        }

        // Check for performance keywords
        if task_lower.contains("performance") || task_lower.contains("optimize") ||
           task_lower.contains("speed") || task_lower.contains("memory") {
            return InstanceRole::PerformanceOptimizer;
        }

        // Check for code generation keywords
        if task_lower.contains("generate") || task_lower.contains("create") ||
           task_lower.contains("implement") || task_lower.contains("add") {
            return InstanceRole::CodeGenerator;
        }

        // Check if this is a complex task that might need meta-orchestration
        if relevant_files.len() > 10 || task_description.len() > 1000 ||
           task_lower.contains("multiple") || task_lower.contains("across") {
            return InstanceRole::MetaOrchestrator;
        }

        InstanceRole::GeneralWorker
    }

    /// Create instance configuration based on role
    fn create_instance_config(&self, role: &InstanceRole) -> InstanceConfig {
        let role_config = match role {
            InstanceRole::TestSpecialist => RoleConfig::TestSpecialist {
                test_frameworks: vec!["cargo test".to_string(), "pytest".to_string()],
                coverage_threshold: 80.0,
            },
            InstanceRole::DocumentationSpecialist => RoleConfig::DocumentationSpecialist {
                doc_formats: vec!["markdown".to_string(), "rustdoc".to_string()],
                update_existing: true,
            },
            InstanceRole::RefactoringSpecialist => RoleConfig::RefactoringSpecialist {
                max_complexity_score: 15.0,
                preserve_api: true,
            },
            InstanceRole::CodeGenerator => RoleConfig::CodeGenerator {
                templates_path: None,
                style_guides: vec!["rustfmt".to_string()],
            },
            InstanceRole::BugFixer => RoleConfig::BugFixer {
                max_risk_level: RiskLevel::Medium,
                require_tests: true,
            },
            InstanceRole::PerformanceOptimizer => RoleConfig::PerformanceOptimizer {
                target_metrics: vec!["latency".to_string(), "memory".to_string()],
                profiling_enabled: true,
            },
            InstanceRole::MetaOrchestrator => RoleConfig::MetaOrchestrator {
                max_sub_instances: 4,
                delegation_strategy: DelegationStrategy::ByComplexity,
            },
            InstanceRole::GeneralWorker => RoleConfig::GeneralWorker {
                max_file_changes: 20,
            },
        };

        InstanceConfig {
            role_config,
            llm_preferences: LlmPreferences::for_role(role),
            allowed_tools: self.get_allowed_tools_for_role(role),
            max_execution_time_seconds: self.get_max_execution_time_for_role(role),
            resource_limits: ResourceLimits::for_role(role),
        }
    }

    /// Build command to spawn a safe-coder instance
    fn build_instance_command(
        &self,
        config: &InstanceConfig,
        workspace_path: &PathBuf,
    ) -> Result<Command> {
        let exe_path = std::env::current_exe()?;
        let mut cmd = Command::new(&exe_path);

        cmd.current_dir(workspace_path);

        // Add configuration flags based on role
        match &config.role_config {
            RoleConfig::TestSpecialist { coverage_threshold, .. } => {
                cmd.arg("--test-mode")
                   .arg("--coverage-threshold")
                   .arg(coverage_threshold.to_string());
            }
            RoleConfig::DocumentationSpecialist { update_existing, .. } => {
                cmd.arg("--doc-mode");
                if *update_existing {
                    cmd.arg("--update-existing-docs");
                }
            }
            RoleConfig::RefactoringSpecialist { preserve_api, .. } => {
                cmd.arg("--refactor-mode");
                if *preserve_api {
                    cmd.arg("--preserve-api");
                }
            }
            RoleConfig::BugFixer { require_tests, .. } => {
                cmd.arg("--bug-fix-mode");
                if *require_tests {
                    cmd.arg("--require-tests");
                }
            }
            RoleConfig::PerformanceOptimizer { profiling_enabled, .. } => {
                cmd.arg("--optimize-mode");
                if *profiling_enabled {
                    cmd.arg("--enable-profiling");
                }
            }
            RoleConfig::MetaOrchestrator { max_sub_instances, .. } => {
                cmd.arg("--orchestrator-mode")
                   .arg("--max-sub-instances")
                   .arg(max_sub_instances.to_string());
            }
            RoleConfig::GeneralWorker { .. } => {
                // No special flags for general worker
            }
            RoleConfig::CodeGenerator { .. } => {
                cmd.arg("--generate-mode");
            }
        }

        // Add LLM preferences
        if let Some(provider) = &config.llm_preferences.preferred_provider {
            cmd.arg("--llm-provider").arg(provider);
        }

        if let Some(temperature) = config.llm_preferences.temperature {
            cmd.arg("--temperature").arg(temperature.to_string());
        }

        // Add resource limits
        if let Some(max_time) = config.max_execution_time_seconds.into() {
            cmd.arg("--timeout").arg(max_time.to_string());
        }

        Ok(cmd)
    }

    /// Get allowed tools for a specific role
    fn get_allowed_tools_for_role(&self, role: &InstanceRole) -> Vec<String> {
        match role {
            InstanceRole::TestSpecialist => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "bash".to_string(), // For running tests
                "glob".to_string(),
                "grep".to_string(),
            ],
            InstanceRole::DocumentationSpecialist => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "webfetch".to_string(), // For external documentation
            ],
            InstanceRole::RefactoringSpecialist => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "edit_file".to_string(),
                "ast_grep".to_string(), // For code analysis
                "glob".to_string(),
            ],
            InstanceRole::BugFixer => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "edit_file".to_string(),
                "bash".to_string(), // For running tests/debugging
                "grep".to_string(),
                "glob".to_string(),
            ],
            InstanceRole::PerformanceOptimizer => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "edit_file".to_string(),
                "bash".to_string(), // For benchmarking
                "ast_grep".to_string(),
            ],
            InstanceRole::CodeGenerator => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "glob".to_string(),
                "ast_grep".to_string(),
            ],
            InstanceRole::MetaOrchestrator => vec![
                // Meta orchestrators have access to all tools
                "read_file".to_string(),
                "write_file".to_string(),
                "edit_file".to_string(),
                "bash".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "ast_grep".to_string(),
                "webfetch".to_string(),
                "subagent".to_string(), // For spawning sub-instances
            ],
            InstanceRole::GeneralWorker => vec![
                "read_file".to_string(),
                "write_file".to_string(),
                "edit_file".to_string(),
                "bash".to_string(),
                "glob".to_string(),
                "grep".to_string(),
            ],
        }
    }

    /// Get maximum execution time for a specific role
    fn get_max_execution_time_for_role(&self, role: &InstanceRole) -> u64 {
        match role {
            InstanceRole::TestSpecialist => 1800, // 30 minutes (tests can take time)
            InstanceRole::DocumentationSpecialist => 900, // 15 minutes
            InstanceRole::RefactoringSpecialist => 2400, // 40 minutes (complex refactoring)
            InstanceRole::BugFixer => 1200, // 20 minutes
            InstanceRole::PerformanceOptimizer => 3600, // 60 minutes (benchmarking takes time)
            InstanceRole::CodeGenerator => 600, // 10 minutes
            InstanceRole::MetaOrchestrator => 7200, // 2 hours (managing others)
            InstanceRole::GeneralWorker => 1800, // 30 minutes
        }
    }

    /// Get status of all active instances
    pub fn get_active_instances(&self) -> &HashMap<String, SelfOrchestratedInstance> {
        &self.active_instances
    }

    /// Stop a specific instance
    pub async fn stop_instance(&mut self, instance_id: &str) -> Result<()> {
        if let Some(instance) = self.active_instances.remove(instance_id) {
            if let Some(pid) = instance.process_id {
                let _ = Command::new("kill")
                    .arg(pid.to_string())
                    .output()
                    .await;
            }
        }
        Ok(())
    }

    /// Stop all instances
    pub async fn stop_all_instances(&mut self) -> Result<()> {
        let instance_ids: Vec<String> = self.active_instances.keys().cloned().collect();
        for instance_id in instance_ids {
            self.stop_instance(&instance_id).await?;
        }
        Ok(())
    }
}

impl Default for SelfOrchestrationStrategies {
    fn default() -> Self {
        Self {
            auto_role_detection: true,
            hierarchical_orchestration: true,
            dynamic_load_balancing: true,
            inter_instance_communication: false, // Future feature
        }
    }
}

impl LlmPreferences {
    /// Create LLM preferences optimized for a specific role
    fn for_role(role: &InstanceRole) -> Self {
        match role {
            InstanceRole::TestSpecialist => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.1), // Low creativity for tests
                max_tokens: Some(4000),
            },
            InstanceRole::DocumentationSpecialist => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.3), // Some creativity for docs
                max_tokens: Some(8000),
            },
            InstanceRole::RefactoringSpecialist => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.1), // Low creativity for refactoring
                max_tokens: Some(6000),
            },
            InstanceRole::CodeGenerator => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.4), // Some creativity for generation
                max_tokens: Some(6000),
            },
            InstanceRole::BugFixer => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.1), // Low creativity for bug fixes
                max_tokens: Some(4000),
            },
            InstanceRole::PerformanceOptimizer => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.2), // Low creativity for optimization
                max_tokens: Some(6000),
            },
            InstanceRole::MetaOrchestrator => Self {
                preferred_provider: Some("claude".to_string()),
                model_preferences: vec!["claude-3-5-sonnet-20241022".to_string()],
                temperature: Some(0.3), // Balanced for planning
                max_tokens: Some(8000),
            },
            InstanceRole::GeneralWorker => Self {
                preferred_provider: None,
                model_preferences: vec![],
                temperature: Some(0.3),
                max_tokens: Some(4000),
            },
        }
    }
}

impl ResourceLimits {
    /// Create resource limits appropriate for a specific role
    fn for_role(role: &InstanceRole) -> Self {
        match role {
            InstanceRole::TestSpecialist => Self {
                max_memory_mb: Some(2048), // Tests can use memory
                max_cpu_percent: Some(80.0),
                max_disk_mb: Some(1024),
            },
            InstanceRole::DocumentationSpecialist => Self {
                max_memory_mb: Some(512), // Documentation is lightweight
                max_cpu_percent: Some(30.0),
                max_disk_mb: Some(512),
            },
            InstanceRole::RefactoringSpecialist => Self {
                max_memory_mb: Some(1536), // Moderate resource usage
                max_cpu_percent: Some(60.0),
                max_disk_mb: Some(1024),
            },
            InstanceRole::CodeGenerator => Self {
                max_memory_mb: Some(1024),
                max_cpu_percent: Some(50.0),
                max_disk_mb: Some(1024),
            },
            InstanceRole::BugFixer => Self {
                max_memory_mb: Some(1024),
                max_cpu_percent: Some(50.0),
                max_disk_mb: Some(512),
            },
            InstanceRole::PerformanceOptimizer => Self {
                max_memory_mb: Some(3072), // May need more resources for benchmarking
                max_cpu_percent: Some(90.0),
                max_disk_mb: Some(2048),
            },
            InstanceRole::MetaOrchestrator => Self {
                max_memory_mb: Some(1024), // Orchestrators are lightweight
                max_cpu_percent: Some(40.0),
                max_disk_mb: Some(512),
            },
            InstanceRole::GeneralWorker => Self {
                max_memory_mb: Some(1024),
                max_cpu_percent: Some(50.0),
                max_disk_mb: Some(1024),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_role_detection() {
        let manager = SelfOrchestrationManager::new(OrchestratorConfig::default());

        // Test role detection
        assert_eq!(
            manager.detect_optimal_role("Write tests for the auth module", &["src/auth/test.rs".to_string()]),
            InstanceRole::TestSpecialist
        );

        assert_eq!(
            manager.detect_optimal_role("Update the README with new API docs", &["README.md".to_string()]),
            InstanceRole::DocumentationSpecialist
        );

        assert_eq!(
            manager.detect_optimal_role("Refactor the authentication system", &[]),
            InstanceRole::RefactoringSpecialist
        );

        assert_eq!(
            manager.detect_optimal_role("Fix the bug in user login", &[]),
            InstanceRole::BugFixer
        );
    }

    #[test]
    fn test_instance_config_creation() {
        let manager = SelfOrchestrationManager::new(OrchestratorConfig::default());

        let config = manager.create_instance_config(&InstanceRole::TestSpecialist);
        
        match config.role_config {
            RoleConfig::TestSpecialist { coverage_threshold, .. } => {
                assert_eq!(coverage_threshold, 80.0);
            }
            _ => panic!("Expected TestSpecialist config"),
        }

        assert!(config.allowed_tools.contains(&"bash".to_string()));
        assert_eq!(config.max_execution_time_seconds, 1800);
    }
}