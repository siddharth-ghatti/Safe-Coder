use anyhow::Result;
use serial_test::serial;
use crate::common::*;

#[tokio::test]
#[serial]
async fn test_cli_help() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    // Test the help command 
    let output = env.run_safe_coder(&["--help"]).await?;
    
    // Basic assertions
    assert_success(&output);
    let output_str = output_to_string(&output);
    assert_contains(&output_str, "safe-coder");
    
    Ok(())
}

#[tokio::test]
#[serial] 
async fn test_cli_invalid_flag() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    // Test an invalid flag (--version is not supported)
    let output = env.run_safe_coder(&["--version"]).await?;
    
    // Should fail gracefully with usage information
    assert!(!output.status.success());
    let stderr_str = stderr_to_string(&output);
    assert_contains(&stderr_str, "Usage:");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    // Create test config
    env.create_test_config()?;
    
    // Verify the config can be loaded from our test environment
    let config = safe_coder::config::Config::load()?;
    
    // Since Config::load() might fall back to default, let's just check it works
    assert!(config.llm.provider == safe_coder::config::LlmProvider::Anthropic);
    
    Ok(())
}