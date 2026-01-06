# Safe-Coder Self-Orchestration Guide

Safe-Coder has advanced self-orchestration capabilities that allow it to coordinate multiple instances of itself to handle complex development tasks in parallel. This guide explains how to use these capabilities effectively.

## Overview

Self-orchestration in Safe-Coder means that one instance of safe-coder can:

1. **Analyze complex tasks** and break them down into parallel subtasks
2. **Spawn multiple instances** of itself in isolated git worktrees
3. **Assign specialized roles** to different instances (testing, documentation, refactoring, etc.)
4. **Coordinate execution** across multiple instances with proper dependency management
5. **Merge results** back into the main branch when complete

## Architecture

### Core Components

1. **Orchestrator**: Main coordination layer that manages the overall workflow
2. **Self-Orchestration Manager**: Specialized manager for safe-coder instance coordination
3. **Workspace Manager**: Handles git worktrees for isolation
4. **Specialized Roles**: Different instance types optimized for specific tasks

### Instance Roles

Each self-orchestrated instance can take on specialized roles:

- **GeneralWorker**: Standard safe-coder functionality
- **TestSpecialist**: Optimized for writing and running tests
- **DocumentationSpecialist**: Focused on documentation tasks
- **RefactoringSpecialist**: Specialized for large-scale code refactoring
- **CodeGenerator**: Optimized for generating new code
- **BugFixer**: Specialized for debugging and bug fixes
- **PerformanceOptimizer**: Focused on performance improvements
- **MetaOrchestrator**: Can spawn and manage sub-instances

## Quick Start

### Basic Self-Orchestration

```bash
# Enable self-orchestration for a complex task
safe-coder --self-orchestrate "Implement OAuth2 authentication with Google and GitHub, including tests and documentation"

# Use a specific number of instances
safe-coder --self-orchestrate --max-self-instances 6 "Large refactoring of the API layer"

# Show the orchestration plan without executing
safe-coder --show-self-orchestration-plan "Complex multi-module feature development"
```

### Advanced Configuration

```bash
# Configure self-orchestration settings
safe-coder self-orchestration configure \
    --max-instances 8 \
    --auto-roles true \
    --hierarchical true \
    --strategy task-based \
    --start-delay-ms 300

# Execute with specific roles
safe-coder self-orchestration execute \
    --roles test,documentation,refactoring \
    --mode act \
    "Modernize the authentication system"

# Check status of running instances
safe-coder self-orchestration status

# Stop all instances
safe-coder self-orchestration stop
```

## Configuration Options

### Orchestration Strategies

1. **Single Worker**: Use only one safe-coder instance (traditional mode)
2. **Round Robin**: Distribute tasks evenly across instances
3. **Task-Based**: Assign instances based on task characteristics (recommended)
4. **Load Balanced**: Distribute based on current instance load

### Role Configuration

Each role has specialized settings:

```rust
// Example: Test Specialist configuration
TestSpecialist {
    test_frameworks: ["cargo test", "pytest"],
    coverage_threshold: 80.0,
    max_execution_time: 30 minutes,
    allowed_tools: ["read_file", "write_file", "bash", "glob", "grep"]
}

// Example: Documentation Specialist configuration  
DocumentationSpecialist {
    doc_formats: ["markdown", "rustdoc"],
    update_existing: true,
    max_execution_time: 15 minutes,
    allowed_tools: ["read_file", "write_file", "glob", "grep", "webfetch"]
}
```

## Use Cases

### 1. Large Feature Development

Perfect for implementing complex features that span multiple modules:

```bash
safe-coder --self-orchestrate "
Implement a new user notification system with:
- Real-time WebSocket notifications
- Email notification service  
- SMS integration
- User preference management
- Admin dashboard
- Comprehensive tests
- API documentation
"
```

This will automatically:
- Detect that multiple specialists are needed
- Spawn instances for backend, frontend, testing, and documentation
- Coordinate development across isolated worktrees
- Merge results when complete

### 2. Large-Scale Refactoring

When you need to refactor across many files simultaneously:

```bash
safe-coder --self-orchestrate "
Refactor the entire API layer to use async/await:
- Convert all controllers from sync to async
- Update database access patterns
- Modify middleware for async compatibility
- Update error handling
- Preserve all existing functionality
- Update tests to async patterns
"
```

### 3. Bug Hunt Across Codebase

Fix multiple related issues in parallel:

```bash
safe-coder --self-orchestrate "
Fix all authentication-related issues:
- Password reset not working
- Session timeout too aggressive  
- OAuth callback errors
- Token refresh failures
- Add logging for all auth events
- Improve error messages
"
```

### 4. Documentation Overhaul

Update documentation across the entire project:

```bash
safe-coder self-orchestration execute \
    --roles documentation,documentation,documentation \
    --instances 3 \
    "Update all documentation for the new API version"
```

## Best Practices

### 1. Task Decomposition

Write requests that clearly describe:
- **What needs to be done** in each area
- **Dependencies** between different parts
- **Files or modules** that will be affected
- **Acceptance criteria** for completion

### 2. Resource Management

- **Start with fewer instances** (2-4) and scale up as needed
- **Monitor system resources** during large orchestrations  
- **Use staggered starts** to avoid overwhelming the system
- **Set appropriate timeouts** for different task types

### 3. Git Workflow

- **Ensure clean working directory** before starting
- **Review merge conflicts** manually if they occur
- **Use meaningful branch names** (auto-generated by instance ID)
- **Consider running tests** after orchestration completes

### 4. Error Handling

- **Failed instances** are automatically cleaned up
- **Partial failures** don't affect successful instances
- **Rollback options** are available via git
- **Logs are preserved** for debugging

## Advanced Features

### Hierarchical Orchestration

Meta-orchestrator instances can spawn their own sub-instances:

```bash
safe-coder --self-orchestrate --hierarchical true "
Implement a complete microservices architecture:
- Break down monolith into services
- Set up inter-service communication
- Implement service discovery
- Add monitoring and logging
- Create deployment pipelines
- Comprehensive testing strategy
"
```

The meta-orchestrator will:
1. Plan the high-level architecture
2. Spawn specialist instances for each service
3. Each specialist may spawn its own sub-instances for testing, etc.
4. Coordinate all work through the hierarchy

### Dynamic Load Balancing

Instances automatically balance work based on:
- Current CPU and memory usage
- Task queue depth
- Estimated task completion time
- Instance specialization match

### Cross-Instance Communication

(Future feature) Instances will be able to:
- Share context and decisions
- Coordinate on shared resources
- Resolve conflicts automatically
- Learn from each other's work

## Monitoring and Debugging

### Status Monitoring

```bash
# Check all active instances
safe-coder self-orchestration status

# Monitor resource usage
safe-coder self-orchestration status --resources

# View instance logs
safe-coder self-orchestration logs <instance-id>
```

### Debugging Failed Orchestrations

1. **Check instance logs** for error details
2. **Review workspace states** in `.safe-coder-workspaces/`
3. **Examine git history** for partial work
4. **Use manual merge** if needed for recovery

## Configuration Files

### Global Configuration

```toml
# ~/.safe-coder/orchestration.toml
[self_orchestration]
max_instances = 6
auto_role_detection = true
hierarchical_orchestration = true
worker_strategy = "task-based"
start_delay_ms = 300
use_worktrees = true

[resource_limits]
max_memory_per_instance_mb = 2048
max_cpu_percent_per_instance = 60.0
max_total_instances = 10

[role_preferences]
test_specialist_coverage_threshold = 85.0
documentation_update_existing = true
refactoring_preserve_api = true
```

### Project-Specific Configuration

```toml
# .safe-coder/orchestration.toml (in project root)
[project_orchestration]
preferred_roles = ["test", "documentation", "refactoring"]
max_instances = 4
exclude_paths = ["vendor/", "node_modules/"]
test_command = "cargo test --all"
doc_command = "cargo doc --no-deps"
```

## Performance Considerations

### System Requirements

- **Memory**: 2GB+ per instance (configurable)
- **CPU**: Multi-core recommended (4+ cores for 4+ instances)
- **Disk**: Fast SSD for git operations and workspace management
- **Network**: Good bandwidth if using cloud LLM providers

### Optimization Tips

1. **Limit concurrent instances** based on your system
2. **Use faster LLM providers** for time-sensitive work
3. **Configure appropriate timeouts** to avoid hanging
4. **Monitor resource usage** and adjust limits accordingly
5. **Use worktree cleanup** to manage disk space

## Troubleshooting

### Common Issues

1. **"Too many instances"**: Reduce `max_instances` setting
2. **"Workspace conflicts"**: Clean working directory before starting  
3. **"Merge conflicts"**: Review and resolve manually, then continue
4. **"Instance timeout"**: Increase timeout for complex tasks
5. **"Resource exhaustion"**: Lower resource limits or instance count

### Recovery Procedures

1. **Stop all instances**: `safe-coder self-orchestration stop --force`
2. **Clean workspaces**: Remove `.safe-coder-workspaces/` directory
3. **Reset git state**: `git checkout main && git clean -fd`
4. **Restart with lower limits**: Reduce instance count and retry

## Examples and Demos

### Run Built-in Demos

```bash
# Feature development demo
safe-coder self-orchestration demo feature-development

# Large refactoring demo  
safe-coder self-orchestration demo large-refactoring

# Bug hunt demo
safe-coder self-orchestration demo bug-hunt

# Performance optimization demo
safe-coder self-orchestration demo performance

# Documentation update demo
safe-coder self-orchestration demo documentation
```

### Custom Orchestration Examples

See the `examples/` directory for:
- `self_orchestration_config.rs` - Configuration examples
- `self_orchestration_demo.rs` - Usage examples
- Real-world orchestration scenarios

## Integration with Other Tools

Safe-Coder's self-orchestration works well with:

- **CI/CD pipelines**: For automated large-scale updates
- **Code review tools**: Generate comprehensive PR descriptions
- **Project management**: Break down epics into parallel tasks
- **Testing frameworks**: Comprehensive test generation
- **Documentation tools**: Keep docs in sync with code

This self-orchestration capability makes Safe-Coder particularly powerful for:
- Large codebases with multiple modules
- Complex feature development
- Major refactoring projects  
- Comprehensive testing initiatives
- Documentation maintenance
- Bug fixing across multiple areas

The key is that Safe-Coder can "think" at a higher level about your entire project and coordinate multiple specialized instances to work on different aspects simultaneously, just like a senior developer would coordinate a team of specialists.