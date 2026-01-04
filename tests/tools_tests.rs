use anyhow::Result;
use safe_coder::config::ToolConfig;
use safe_coder::tools::{AgentMode, ToolContext, ToolRegistry};
use tempfile::TempDir;

#[test]
fn test_agent_mode_defaults() {
    let mode = AgentMode::default();
    assert_eq!(mode, AgentMode::Build);
}

#[test]
fn test_agent_mode_enabled_tools() {
    let plan_mode = AgentMode::Plan;
    let build_mode = AgentMode::Build;

    // Plan mode should have read-only tools
    let plan_tools = plan_mode.enabled_tools();
    assert!(plan_tools.contains(&"read_file"));
    assert!(plan_tools.contains(&"list_file"));
    assert!(plan_tools.contains(&"glob"));
    assert!(plan_tools.contains(&"grep"));
    assert!(plan_tools.contains(&"webfetch"));
    assert!(plan_tools.contains(&"todoread"));

    // Plan mode should NOT have write/execute tools
    assert!(!plan_tools.contains(&"write_file"));
    assert!(!plan_tools.contains(&"edit_file"));
    assert!(!plan_tools.contains(&"bash"));
    // Note: subagent is currently disabled in the implementation
    assert!(!plan_tools.contains(&"subagent"));

    // Build mode should have all tools (except subagent which is disabled)
    let build_tools = build_mode.enabled_tools();
    assert!(build_tools.contains(&"read_file"));
    assert!(build_tools.contains(&"write_file"));
    assert!(build_tools.contains(&"edit_file"));
    assert!(build_tools.contains(&"list_file"));
    assert!(build_tools.contains(&"glob"));
    assert!(build_tools.contains(&"grep"));
    assert!(build_tools.contains(&"bash"));
    assert!(build_tools.contains(&"webfetch"));
    assert!(build_tools.contains(&"todowrite"));
    assert!(build_tools.contains(&"todoread"));
    // Note: subagent is currently disabled while perfecting planning
    // assert!(build_tools.contains(&"subagent"));
}

#[test]
fn test_agent_mode_is_tool_enabled() {
    let plan_mode = AgentMode::Plan;
    let build_mode = AgentMode::Build;

    // Test plan mode
    assert!(plan_mode.is_tool_enabled("read_file"));
    assert!(!plan_mode.is_tool_enabled("write_file"));
    assert!(!plan_mode.is_tool_enabled("bash"));

    // Test build mode
    assert!(build_mode.is_tool_enabled("read_file"));
    assert!(build_mode.is_tool_enabled("write_file"));
    assert!(build_mode.is_tool_enabled("bash"));
}

#[test]
fn test_agent_mode_descriptions() {
    let plan_mode = AgentMode::Plan;
    let build_mode = AgentMode::Build;

    assert!(plan_mode.description().contains("Read-only"));
    assert!(plan_mode.description().contains("exploration"));

    assert!(build_mode.description().contains("Full execution"));
    assert!(build_mode.description().contains("modify files"));

    assert_eq!(plan_mode.short_name(), "PLAN");
    assert_eq!(build_mode.short_name(), "BUILD");
}

#[test]
fn test_agent_mode_cycle() {
    let plan_mode = AgentMode::Plan;
    let build_mode = AgentMode::Build;

    assert_eq!(plan_mode.next(), AgentMode::Build);
    assert_eq!(build_mode.next(), AgentMode::Plan);
}

#[test]
fn test_agent_mode_display() {
    let plan_mode = AgentMode::Plan;
    let build_mode = AgentMode::Build;

    let plan_str = format!("{}", plan_mode);
    let build_str = format!("{}", build_mode);

    // Display should include mode names
    assert!(plan_str.to_lowercase().contains("plan") || plan_str.contains("PLAN"));
    assert!(build_str.to_lowercase().contains("build") || build_str.contains("BUILD"));
}

#[tokio::test]
async fn test_tool_registry_creation() -> Result<()> {
    let _temp_dir = TempDir::new()?;

    let _registry = ToolRegistry::new();

    // Basic test - registry should be created without error
    // We can't test much without the actual implementation,
    // but we can verify it doesn't panic

    Ok(())
}

#[test]
fn test_tool_context_creation() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();

    // Create a simple ToolConfig for testing
    let config = ToolConfig {
        bash_timeout_secs: 120,
        max_output_bytes: 1_048_576,
        warn_dangerous_commands: true,
        dangerous_patterns: vec![],
    };

    let context = ToolContext::new(project_path, &config);

    assert_eq!(context.working_dir, project_path);
    assert!(context.output_callback.is_none());
}

#[cfg(test)]
mod file_operations_tests {
    use super::*;

    #[test]
    fn test_tool_context_with_real_paths() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path();

        // Create some test files
        std::fs::write(project_path.join("test.txt"), "Hello, world!")?;
        std::fs::create_dir_all(project_path.join("subdir"))?;
        std::fs::write(project_path.join("subdir/nested.txt"), "Nested content")?;

        let config = ToolConfig {
            bash_timeout_secs: 120,
            max_output_bytes: 1_048_576,
            warn_dangerous_commands: true,
            dangerous_patterns: vec![],
        };

        let context = ToolContext::new(project_path, &config);

        // Verify the context points to valid paths
        assert!(context.working_dir.exists());
        assert!(context.working_dir.join("test.txt").exists());
        assert!(context.working_dir.join("subdir/nested.txt").exists());

        Ok(())
    }
}

#[cfg(test)]
mod mode_compatibility_tests {
    use super::*;

    #[test]
    fn test_mode_tool_compatibility() {
        // Note: subagent is currently disabled while perfecting planning
        let dangerous_tools = ["write_file", "edit_file", "bash"];
        let safe_tools = ["read_file", "list_file", "glob", "grep"];

        let plan_mode = AgentMode::Plan;
        let build_mode = AgentMode::Build;

        // Plan mode should block dangerous tools
        for tool in dangerous_tools.iter() {
            assert!(
                !plan_mode.is_tool_enabled(tool),
                "Plan mode should not enable dangerous tool: {}",
                tool
            );
        }

        // Plan mode should allow safe tools
        for tool in safe_tools.iter() {
            assert!(
                plan_mode.is_tool_enabled(tool),
                "Plan mode should enable safe tool: {}",
                tool
            );
        }

        // Build mode should allow all tools
        for tool in dangerous_tools.iter() {
            assert!(
                build_mode.is_tool_enabled(tool),
                "Build mode should enable tool: {}",
                tool
            );
        }

        for tool in safe_tools.iter() {
            assert!(
                build_mode.is_tool_enabled(tool),
                "Build mode should enable tool: {}",
                tool
            );
        }
    }
}
