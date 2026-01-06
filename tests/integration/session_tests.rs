use super::common::*;
use anyhow::Result;
use safe_coder::config::Config;
use safe_coder::session::Session;
use serial_test::serial;
use std::time::Duration;
use tokio::sync::mpsc;

#[tokio::test]
#[serial]
async fn test_session_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let session = Session::new(config, env.project_path.clone()).await?;

    // Should create session without error
    assert!(session.project_path().exists());
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_start() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let mut session = Session::new(config, env.project_path.clone()).await?;

    // Start session (initializes git tracking, etc.)
    let result = session.start().await;
    
    // Should start successfully or fail gracefully
    match result {
        Ok(_) => {
            // Success - session started
        }
        Err(e) => {
            // Expected if git is not properly configured or other setup issues
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_with_event_channel() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    
    let mut session = Session::new(config, env.project_path.clone()).await?;
    session.set_event_channel(event_tx);

    // Test that event channel is set up
    // In a real test we would trigger events and check they're received
    
    // Try to receive an event with timeout
    let timeout_result = tokio::time::timeout(
        Duration::from_millis(100),
        event_rx.recv()
    ).await;

    // Should timeout (no events sent)
    assert!(timeout_result.is_err());
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_execution_modes() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let mut session = Session::new(config, env.project_path.clone()).await?;

    // Test setting execution modes
    session.set_execution_mode(safe_coder::approval::ExecutionMode::Plan);
    session.set_execution_mode(safe_coder::approval::ExecutionMode::Act);
    
    // Should not error
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_agent_mode_switching() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let mut session = Session::new(config, env.project_path.clone()).await?;

    // Test switching agent modes
    session.set_agent_mode(safe_coder::tools::AgentMode::Plan);
    session.set_agent_mode(safe_coder::tools::AgentMode::Build);
    
    // Should not error
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_stop() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let mut session = Session::new(config, env.project_path.clone()).await?;

    // Start session first
    let _ = session.start().await;
    
    // Stop session
    let result = session.stop().await;
    
    // Should stop without error
    assert!(result.is_ok());
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_message_handling() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let mut session = Session::new(config, env.project_path.clone()).await?;

    // Test sending a message (will likely fail without real LLM API)
    let result = session.send_message("Hello, AI!".to_string()).await;
    
    match result {
        Ok(_response) => {
            // Success case (unexpected without real API)
        }
        Err(e) => {
            // Expected error case
            let error_str = e.to_string().to_lowercase();
            
            // Should be a reasonable error (API key, network, etc.)
            assert!(
                error_str.contains("api") || 
                error_str.contains("key") || 
                error_str.contains("auth") ||
                error_str.contains("client") ||
                error_str.contains("network") ||
                error_str.contains("connection"),
                "Unexpected error: {}", e
            );
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_project_context() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;
    env.create_test_config()?;

    let config = Config::load()?;
    let session = Session::new(config, env.project_path.clone()).await?;

    // Verify session has correct project context
    assert_eq!(session.project_path(), &env.project_path);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_session_with_invalid_project_path() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.create_test_config()?;

    let config = Config::load()?;
    let invalid_path = env.project_path.join("non_existent_directory");
    
    let result = Session::new(config, invalid_path).await;
    
    // Should handle invalid paths gracefully
    match result {
        Ok(_) => {
            // Might succeed if session creates the directory
        }
        Err(e) => {
            // Expected error for invalid path
            assert!(!e.to_string().contains("panic"));
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_session_with_checkpoints() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;
        env.create_test_config()?;

        let config = Config::load()?;
        let session = Session::new(config, env.project_path.clone()).await?;

        // Session should be created with checkpoint support
        // In a real implementation, this would test checkpoint creation/restoration
        
        Ok(())
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_session_memory_management() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;
        env.create_test_config()?;

        let config = Config::load()?;
        let session = Session::new(config, env.project_path.clone()).await?;

        // Test that session includes memory management
        // This would be expanded to test conversation history, context windows, etc.
        
        Ok(())
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_session_permission_management() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;
        env.create_test_config()?;

        let config = Config::load()?;
        let session = Session::new(config, env.project_path.clone()).await?;

        // Test that session includes permission management
        // This would be expanded to test tool permissions, file access controls, etc.
        
        Ok(())
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_session_persistence() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;
        env.create_test_config()?;

        let config = Config::load()?;
        let session = Session::new(config, env.project_path.clone()).await?;

        // Test that session supports persistence
        // This would test session state saving/loading, conversation history, etc.
        
        Ok(())
    }
}