# Integration Tests for Safe-Coder

This directory contains comprehensive integration tests for the safe-coder project. These tests verify end-to-end functionality across all major components.

## Test Structure

### Core Test Modules

- **`cli_tests.rs`** - Tests for command-line interface functionality
  - CLI argument parsing and validation
  - Help and version commands
  - Command execution and error handling
  - Configuration management via CLI

- **`llm_tests.rs`** - Tests for LLM client integrations
  - Client creation for different providers (Anthropic, OpenAI, Ollama, GitHub Copilot)
  - Mock API responses and error handling
  - Provider switching and configuration
  - Caching functionality

- **`orchestrator_tests.rs`** - Tests for orchestrator functionality  
  - Multi-agent task delegation
  - Worker management and strategies
  - Execution modes (Plan vs Act)
  - Workspace isolation and cleanup

- **`session_tests.rs`** - Tests for session management
  - Session lifecycle (creation, start, stop)
  - Event handling and communication
  - Agent mode switching
  - Project context management

- **`tools_tests.rs`** - Tests for tool execution and file operations
  - File read/write/edit operations
  - Shell command execution
  - Tool permission management
  - Agent mode restrictions

- **`git_tests.rs`** - Tests for git integration
  - Repository management
  - Branch operations and worktrees
  - Commit and staging functionality
  - Auto-commit features

- **`config_tests.rs`** - Tests for configuration management
  - Configuration loading and saving
  - Environment variable handling
  - Provider switching
  - Validation and error handling

### Test Utilities

- **`common.rs`** - Shared utilities for integration testing
  - Test environment setup
  - Temporary directory management
  - Mock LLM responses
  - Assertion helpers

## Running Tests

### All Integration Tests
```bash
cargo test --test integration
```

### Specific Test Module
```bash
cargo test --test integration cli_tests
cargo test --test integration llm_tests
cargo test --test integration orchestrator_tests
# etc.
```

### Individual Tests
```bash
cargo test --test integration test_cli_help
cargo test --test integration test_session_creation
# etc.
```

### With Output
```bash
cargo test --test integration -- --nocapture
```

## Test Dependencies

The integration tests require additional development dependencies:

- `tokio-test` - Async test utilities
- `mockito` - HTTP mocking for API tests
- `wiremock` - HTTP mocking framework
- `assert_fs` - Filesystem testing utilities  
- `predicates` - Assertion predicates
- `once_cell` - Lazy static initialization
- `serial_test` - Sequential test execution

## Test Environment

### Isolation
Tests use the `serial_test` attribute to prevent conflicts when:
- Modifying global environment variables
- Creating temporary files
- Running CLI commands
- Accessing shared resources

### Cleanup
Each test creates isolated temporary directories and cleans up resources automatically. Tests should not interfere with each other or leave persistent state.

### Mocking
External dependencies are mocked where possible:
- LLM API calls use mock HTTP responses
- File operations use temporary directories
- Git operations use isolated test repositories

## Test Configuration

### Environment Variables
Tests temporarily modify environment variables and restore them after completion:
- `XDG_CONFIG_HOME` - Test configuration directory
- `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` - API credentials
- `NO_COLOR` - Disable color output for easier testing

### Timeouts
Long-running tests (especially CLI tests) include timeouts to prevent hanging:
- Demo mode tests timeout after 5 seconds
- Interactive commands are tested with non-interactive inputs

## Common Patterns

### Test Structure
```rust
#[tokio::test]
#[serial]
async fn test_functionality() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    
    // Test implementation
    let result = some_operation().await?;
    
    // Assertions
    assert_success(&result);
    assert_contains(&output, "expected content");
    
    Ok(())
}
```

### Error Testing
```rust
match result {
    Ok(output) => {
        // Verify successful output
        assert_contains(&output, "success");
    }
    Err(e) => {
        // Verify graceful error handling
        assert!(!e.to_string().contains("panic"));
        assert_contains(&e.to_string(), "expected error");
    }
}
```

## Contributing

When adding new integration tests:

1. Use the `TestEnvironment` for consistent setup
2. Add `#[serial]` for tests that modify global state
3. Use proper async/await patterns with `#[tokio::test]`
4. Include both success and error test cases
5. Clean up resources (handled automatically by `TestEnvironment`)
6. Mock external dependencies where possible
7. Add descriptive test names and documentation

## Troubleshooting

### Test Failures
- Check that safe-coder binary is built: `cargo build`
- Ensure git is installed and configured
- Verify test isolation with `#[serial]` if needed
- Check for resource conflicts between tests

### Timeout Issues
- Interactive commands may need non-interactive alternatives
- Increase timeout values for slow operations
- Use `tokio::time::timeout` for long-running operations

### Mock Failures
- Verify mock server setup and teardown
- Check HTTP request/response matching
- Ensure proper async handling of mock responses