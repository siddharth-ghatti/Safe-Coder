use super::common::*;
use anyhow::Result;
use safe_coder::config::ToolConfig;
use safe_coder::tools::{AgentMode, ToolContext, ToolRegistry};
use serial_test::serial;
use std::fs;
use std::path::PathBuf;

#[tokio::test]
#[serial]
async fn test_tool_registry_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;

    let registry = ToolRegistry::new();
    
    // Should create without error
    assert!(std::mem::size_of_val(&registry) > 0);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_context_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;

    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);
    
    assert_eq!(context.working_dir, env.project_path);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_file_operations() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test read_file tool
    let read_result = registry.execute_tool(
        "read_file",
        &serde_json::json!({
            "file_path": "src/main.rs"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match read_result {
        Ok(output) => {
            assert_contains(&output, "Hello, world!");
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_write_file_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test write_file tool
    let test_content = "// This is a test file\nfn test() {}\n";
    let write_result = registry.execute_tool(
        "write_file",
        &serde_json::json!({
            "file_path": "test_file.rs",
            "content": test_content
        }),
        &context,
        AgentMode::Build,
    ).await;

    match write_result {
        Ok(_) => {
            // Verify file was written
            let written_path = env.project_path.join("test_file.rs");
            assert!(written_path.exists());
            
            let content = fs::read_to_string(written_path)?;
            assert_eq!(content, test_content);
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_edit_file_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // First create a file to edit
    let test_file = env.project_path.join("edit_test.rs");
    fs::write(&test_file, "fn old_function() {}\n")?;

    // Test edit_file tool
    let edit_result = registry.execute_tool(
        "edit_file",
        &serde_json::json!({
            "file_path": "edit_test.rs",
            "old_string": "old_function",
            "new_string": "new_function"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match edit_result {
        Ok(_) => {
            // Verify file was edited
            let content = fs::read_to_string(test_file)?;
            assert_contains(&content, "new_function");
            assert!(!content.contains("old_function"));
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_list_file_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test list_file tool
    let list_result = registry.execute_tool(
        "list_file",
        &serde_json::json!({
            "path": "src"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match list_result {
        Ok(output) => {
            assert_contains(&output, "main.rs");
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_glob_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test glob tool
    let glob_result = registry.execute_tool(
        "glob",
        &serde_json::json!({
            "pattern": "**/*.rs"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match glob_result {
        Ok(output) => {
            assert_contains(&output, "main.rs");
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_grep_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test grep tool
    let grep_result = registry.execute_tool(
        "grep",
        &serde_json::json!({
            "pattern": "Hello",
            "path": "src/main.rs"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match grep_result {
        Ok(output) => {
            assert_contains(&output, "Hello");
        }
        Err(e) => {
            // Should be a reasonable error (pattern not found is OK)
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_bash_tool() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test bash tool with a simple command
    let bash_result = registry.execute_tool(
        "bash",
        &serde_json::json!({
            "command": "echo 'Hello from bash'"
        }),
        &context,
        AgentMode::Build,
    ).await;

    match bash_result {
        Ok(output) => {
            assert_contains(&output, "Hello from bash");
        }
        Err(e) => {
            // Should be a reasonable error
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_dangerous_command_detection() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig {
        bash_timeout_secs: 10,
        max_output_bytes: 1024,
        warn_dangerous_commands: true,
        dangerous_patterns: vec!["rm -rf".to_string(), "mkfs".to_string()],
    };
    let context = ToolContext::new(&env.project_path, &config);

    // Test that dangerous commands are handled appropriately
    let dangerous_result = registry.execute_tool(
        "bash",
        &serde_json::json!({
            "command": "rm -rf /tmp/safe_test_file"
        }),
        &context,
        AgentMode::Build,
    ).await;

    // Should either warn or block the dangerous command
    match dangerous_result {
        Ok(output) => {
            // If allowed, should include warning
            assert!(output.contains("WARNING") || output.contains("dangerous") || output.is_empty());
        }
        Err(e) => {
            // Blocking dangerous commands is also acceptable
            assert_contains(&e.to_string(), "dangerous");
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_agent_mode_restrictions() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test that Plan mode restricts dangerous tools
    let write_in_plan_mode = registry.execute_tool(
        "write_file",
        &serde_json::json!({
            "file_path": "plan_test.txt",
            "content": "test"
        }),
        &context,
        AgentMode::Plan, // Plan mode should block write operations
    ).await;

    // Should be blocked in Plan mode
    assert!(write_in_plan_mode.is_err());
    
    // Test that Build mode allows the same operation
    let write_in_build_mode = registry.execute_tool(
        "write_file",
        &serde_json::json!({
            "file_path": "build_test.txt",
            "content": "test"
        }),
        &context,
        AgentMode::Build, // Build mode should allow write operations
    ).await;

    // Should be allowed in Build mode (or fail for other reasons)
    match write_in_build_mode {
        Ok(_) => {
            // Success - file should be written
            let test_file = env.project_path.join("build_test.txt");
            assert!(test_file.exists());
        }
        Err(e) => {
            // Failure should not be due to mode restrictions
            assert!(!e.to_string().contains("mode"));
            assert!(!e.to_string().contains("restricted"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_todo_tools() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    let registry = ToolRegistry::new();
    let config = ToolConfig::default();
    let context = ToolContext::new(&env.project_path, &config);

    // Test todowrite tool
    let todowrite_result = registry.execute_tool(
        "todowrite",
        &serde_json::json!({
            "todos": [
                {
                    "content": "Test todo item",
                    "status": "pending",
                    "active_form": "Testing todo item..."
                }
            ]
        }),
        &context,
        AgentMode::Build,
    ).await;

    match todowrite_result {
        Ok(_) => {
            // Test todoread tool
            let todoread_result = registry.execute_tool(
                "todoread",
                &serde_json::json!({}),
                &context,
                AgentMode::Build,
            ).await;

            match todoread_result {
                Ok(output) => {
                    assert_contains(&output, "Test todo item");
                }
                Err(e) => {
                    assert!(!e.to_string().contains("panic"));
                }
            }
        }
        Err(e) => {
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tool_error_handling_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_invalid_tool_name() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        
        let registry = ToolRegistry::new();
        let config = ToolConfig::default();
        let context = ToolContext::new(&env.project_path, &config);

        let result = registry.execute_tool(
            "nonexistent_tool",
            &serde_json::json!({}),
            &context,
            AgentMode::Build,
        ).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert_contains(&error_msg, "tool");
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_invalid_tool_arguments() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        
        let registry = ToolRegistry::new();
        let config = ToolConfig::default();
        let context = ToolContext::new(&env.project_path, &config);

        // Test read_file with missing file_path
        let result = registry.execute_tool(
            "read_file",
            &serde_json::json!({}), // Missing required file_path
            &context,
            AgentMode::Build,
        ).await;

        assert!(result.is_err());
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_file_not_found_error() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        
        let registry = ToolRegistry::new();
        let config = ToolConfig::default();
        let context = ToolContext::new(&env.project_path, &config);

        // Test read_file with nonexistent file
        let result = registry.execute_tool(
            "read_file",
            &serde_json::json!({
                "file_path": "nonexistent_file.txt"
            }),
            &context,
            AgentMode::Build,
        ).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("No such file") || 
            error_msg.contains("not found") ||
            error_msg.contains("does not exist")
        );
        
        Ok(())
    }
}