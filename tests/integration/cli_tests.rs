use super::common::*;
use anyhow::Result;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_cli_help() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["--help"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should contain basic help information
    assert_contains(&stdout, "safe-coder");
    assert_contains(&stdout, "AI coding orchestrator");
    assert_contains(&stdout, "COMMANDS");
    assert_contains(&stdout, "shell");
    assert_contains(&stdout, "chat");
    assert_contains(&stdout, "orchestrate");
    assert_contains(&stdout, "config");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_cli_version() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["--version"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should contain version information
    assert_contains(&stdout, "safe-coder");
    assert_contains(&stdout, "0.1.0");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_init_command() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["init", env.project_path.to_str().unwrap()]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should indicate successful initialization
    assert_contains(&stdout, "Initialized safe-coder project");
    assert_contains(&stdout, "Next steps");
    assert_contains(&stdout, "Configure your API key");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_show() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.create_test_config()?;
    
    let output = env.run_safe_coder(&["config", "--show"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should show current configuration
    assert_contains(&stdout, "Current configuration");
    assert_contains(&stdout, "[llm]");
    assert_contains(&stdout, "[git]");
    assert_contains(&stdout, "[orchestrator]");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_set_model() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.create_test_config()?;
    
    let output = env.run_safe_coder(&["config", "--model", "gpt-4"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should indicate model was updated
    assert_contains(&stdout, "Model updated");
    assert_contains(&stdout, "Configuration saved");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_orchestrate_help() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["orchestrate", "--help"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should contain orchestrate command help
    assert_contains(&stdout, "Orchestrate a task");
    assert_contains(&stdout, "--task");
    assert_contains(&stdout, "--worker");
    assert_contains(&stdout, "--max-workers");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_chat_demo_mode() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    // Run in demo mode with a timeout to prevent hanging
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        env.run_safe_coder(&["chat", "--demo", "--path", env.project_path.to_str().unwrap()])
    ).await;
    
    // Demo mode should start successfully or timeout (which is expected for interactive mode)
    match output {
        Ok(result) => {
            let result = result?;
            // If it exits immediately, check for expected output
            let stdout = output_to_string(&result);
            let stderr = stderr_to_string(&result);
            
            // Should not crash with obvious errors
            assert!(!stderr.contains("panic"));
            assert!(!stderr.contains("Error"));
        }
        Err(_) => {
            // Timeout is expected for interactive mode - this is OK
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_shell_help() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["shell", "--help"]).await?;
    
    assert_success(&output);
    let stdout = output_to_string(&output);
    
    // Should contain shell command help
    assert_contains(&stdout, "Start an interactive shell");
    assert_contains(&stdout, "--ai");
    assert_contains(&stdout, "--no-tui");
    assert_contains(&stdout, "--path");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_invalid_command() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let output = env.run_safe_coder(&["invalid-command"]).await?;
    
    // Should fail with helpful error
    assert!(!output.status.success());
    let stderr = stderr_to_string(&output);
    
    // Should suggest valid commands
    assert!(stderr.contains("error") || stderr.contains("unrecognized"));
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_project_path_validation() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    // Test with non-existent path
    let output = env.run_safe_coder(&[
        "chat", 
        "--path", 
        "/non/existent/path",
        "--demo"
    ]).await?;
    
    // Should handle non-existent paths gracefully
    if !output.status.success() {
        let stderr = stderr_to_string(&output);
        assert_contains(&stderr, "No such file or directory");
    }
    
    Ok(())
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_missing_required_args() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        // Test orchestrate without task in non-interactive mode
        let output = env.run_safe_coder(&["orchestrate", "--worker", "claude"]).await;
        
        // This should either work (interactive mode) or fail gracefully
        match output {
            Ok(result) => {
                // If it succeeds, it should enter interactive mode
                let stdout = output_to_string(&result);
                let stderr = stderr_to_string(&result);
                
                // Should not panic
                assert!(!stderr.contains("panic"));
            }
            Err(_) => {
                // Expected if command requires more arguments
            }
        }
        
        Ok(())
    }

    #[tokio::test] 
    #[serial]
    async fn test_invalid_worker_type() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        let output = env.run_safe_coder(&[
            "orchestrate", 
            "--worker", 
            "invalid-worker",
            "--task",
            "test task"
        ]).await?;
        
        // Should handle invalid worker types gracefully
        // The application should either default to a valid worker or show an error
        let stdout = output_to_string(&output);
        let stderr = stderr_to_string(&output);
        
        // Should not crash
        assert!(!stderr.contains("panic"));
        
        Ok(())
    }
}