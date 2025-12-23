# Qwen Code CLI-Inspired Features

This document describes all the new features added to Safe Coder to match the functionality of Qwen Code CLI while maintaining git-based workspace isolation.

## Overview

Safe Coder now includes a comprehensive command system inspired by Qwen Code CLI, providing:
- **Slash Commands** for meta-level control
- **At-Commands** for file context attachment
- **Shell Passthrough** for direct command execution
- **Session Management** for saving and resuming conversations
- **Approval Modes** for controlling tool execution
- **Memory/Instruction System** for project-specific context
- **Statistics Tracking** for token usage and tool calls
- **Custom Commands** for user-defined shortcuts

## Command Types

### 1. Slash Commands (/)

Slash commands provide meta-level control over Safe Coder:

#### Basic Commands
- `/help` or `/?` - Show available commands
- `/quit` or `/exit` - Exit the session
- `/clear` - Clear the screen
- `/stats` - Show session statistics (tokens, time, tool usage)
- `/about` - Show version and information

#### Session Management
- `/chat save [name]` - Save current conversation
- `/chat resume <id>` - Resume a saved conversation
- `/chat list` - List all saved conversations
- `/chat delete <id>` - Delete a saved conversation

#### Memory & Context
- `/memory add <text>` - Add instruction to memory
- `/memory show` - Show current memory/instructions
- `/memory refresh` - Reload from SAFE_CODER.md file
- `/init` - Create project context file (.safe-coder/SAFE_CODER.md)

#### Configuration
- `/model [name]` - Switch model or show current model
- `/approval-mode [mode]` - Set approval mode (see below)
- `/settings` - Show current settings

#### Project Tools
- `/summary` - Generate project summary
- `/compress` - Compress conversation to save tokens
- `/restore [file]` - Restore file(s) from checkpoint
- `/tools` - List available tools
- `/dir add <path>` - Add directory to workspace
- `/dir show` - Show workspace directories

#### Other
- `/copy` - Copy last output to clipboard

### 2. At-Commands (@)

At-commands attach file contents to your message for context:

```bash
# Attach a single file
@main.rs

# Attach multiple files
@src/main.rs @src/config.rs

# Use glob patterns
@src/**/*.rs

# Embed in natural language
"Check @main.rs for errors"
"Review @src/**/*.rs for code quality"
```

### 3. Shell Passthrough (!)

Execute shell commands directly in the isolated sandbox:

```bash
!ls -la
!cargo build
!git status
!pytest tests/
```

## Approval Modes

Control how tool execution is handled:

### `plan` Mode
- Shows execution plan before running
- Asks for confirmation
- Best for understanding what will happen

### `default` Mode (Default)
- Asks before each tool use
- Good balance of control and convenience
- Recommended for general use

### `auto-edit` Mode
- Auto-approves file operations (read, write, edit)
- Asks for approval on bash and other tools
- Good for rapid iteration on code

### `yolo` Mode
- Auto-approves everything
- **Use with caution!**
- Only for trusted operations

Example:
```bash
/approval-mode auto-edit
```

## Session Management

### Saving Conversations

Save your current conversation for later:

```bash
/chat save "feature-authentication"
```

This saves:
- All messages
- Project path
- Timestamp

### Resuming Conversations

Resume a previous conversation:

```bash
# List all saved sessions
/chat list

# Resume a specific session
/chat resume abc-123-def
```

### Managing Sessions

```bash
# Delete a session
/chat delete abc-123-def
```

## Memory & Instructions

### Project Context File

Create a `.safe-coder/SAFE_CODER.md` file in your project:

```bash
/init
```

This file is automatically loaded and included in every conversation. Use it for:
- Project overview
- Code style guidelines
- Important conventions
- Files to focus on
- Things to avoid

Example `SAFE_CODER.md`:
```markdown
# Project Context for Safe Coder

## Project Overview
This is a REST API built with Actix-web and PostgreSQL.

## Code Style Guidelines
- Use async/await for all I/O operations
- Follow Rust naming conventions
- Write comprehensive tests for all handlers

## Important Conventions
- Database migrations in `migrations/` directory
- All handlers return Result<HttpResponse, Error>

## Files to Focus On
- src/handlers/ - API endpoint handlers
- src/models/ - Database models
- src/middleware/ - Custom middleware

## Things to Avoid
- Don't use unwrap() in production code
- Avoid blocking operations in async contexts
```

### Runtime Instructions

Add instructions during the session:

```bash
/memory add "Always add unit tests for new functions"
/memory add "Use serde for serialization"

# View current memory
/memory show
```

## Statistics Tracking

Track your session statistics:

```bash
/stats
```

Shows:
- Session duration
- Total messages sent
- Tool calls executed
- Token usage (approximate)
- Tools used with counts

Example output:
```
📊 Session Statistics
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

⏱️  Duration: 0h 15m 42s
💬 Messages: 23
🔧 Tool Calls: 47

📝 Token Usage:
   Sent:         8,432
   Received:     6,789
   Total:       15,221

🔨 Tools Used:
   write_file      12
   read_file       18
   edit_file       15
   bash             2
```

## Custom Commands

Create your own slash commands for frequently used operations.

### Global Commands

Create in `~/.config/safe-coder/commands/`:

```bash
mkdir -p ~/.config/safe-coder/commands
```

### Project Commands

Create in `.safe-coder/commands/` (project-specific).

### Command Format

Commands are TOML files:

```toml
# ~/.config/safe-coder/commands/test.toml
description = "Run project tests"
prompt = "Run the test suite and show me the results: !cargo test {{args}}"
```

Usage:
```bash
/test
/test --verbose
```

### Advanced Custom Commands

Support for argument injection:

```toml
# refactor.toml
description = "Refactor a function"
prompt = "Refactor the function {{args}} to be more idiomatic and add error handling"
```

Usage:
```bash
/refactor calculate_total
```

## File Restoration

Safe Coder automatically creates checkpoints before file modifications.

### Restore All Files

```bash
/restore
```

### Restore Specific File

```bash
/restore src/main.rs
```

## Conversation Compression

When conversations get long and consume too many tokens:

```bash
/compress
```

This keeps the most recent messages and discards older context, saving tokens.

## Integration with Git Workspace Isolation

All commands work with git-based change tracking:

- **At-commands** read files from the project directory
- **Shell passthrough** executes in the project directory
- **File restoration** works with git checkpoints
- **All AI operations** are tracked with automatic git commits

## Security Notes

1. **Sandbox First**: All tool execution happens in the isolated sandbox
2. **No Host Access**: Commands cannot access your host filesystem directly
3. **Git Tracking**: All changes are tracked in git within the sandbox
4. **Approval Modes**: Control what gets executed automatically
5. **Review Before Sync**: Changes are synced to host only on exit

## Examples

### Example 1: Code Review Workflow

```bash
# Attach files for review
@src/**/*.rs

# AI reviews the code
"Review these files for potential bugs and code quality issues"

# Save the session
/chat save "code-review-2025-01-15"
```

### Example 2: Debug Session

```bash
# Check what's happening
!cargo build

# Show build errors, attach source
@src/main.rs

# Ask AI to fix
"Fix the compilation errors shown above"

# Verify
!cargo test

# Show stats
/stats
```

### Example 3: Refactoring with Safety

```bash
# Set approval mode
/approval-mode auto-edit

# Add instructions
/memory add "Keep all existing tests passing"
/memory add "Add comments for complex logic"

# Attach files
@src/legacy_code.rs

# Request refactoring
"Refactor this code to use modern Rust idioms"

# If something goes wrong
/restore src/legacy_code.rs
```

## Comparison with Qwen Code CLI

| Feature | Qwen Code | Safe Coder | Notes |
|---------|-----------|------------|-------|
| Slash Commands | ✅ | ✅ | Full support |
| At-Commands | ✅ | ✅ | With glob patterns |
| Shell Passthrough | ✅ | ✅ | In sandbox |
| Session Management | ✅ | ✅ | SQLite storage |
| Custom Commands | ✅ | ✅ | TOML format |
| Memory/Instructions | ✅ | ✅ | SAFE_CODER.md |
| Approval Modes | ✅ | ✅ | 4 modes |
| Statistics | ✅ | ✅ | Token tracking |
| MCP Support | ✅ | ⏳ | Coming soon |
| Extensions | ✅ | ⏳ | Coming soon |
| Git Workspace Isolation | ❌ | ✅ | **Safe Coder exclusive** |
| Git Tracking | Partial | ✅ | **Auto-commit after tools** |
| File Restoration | Limited | ✅ | **Checkpoint system** |

## What's Next

Planned features:
- [ ] MCP (Model Context Protocol) server support
- [ ] Extension system
- [ ] Conversation branching
- [ ] Better project analysis for /summary
- [ ] Syntax highlighting in TUI
- [ ] Interactive diff viewer
- [ ] Clipboard integration for /copy

## Getting Help

```bash
# Show all commands
/help

# Get started
/init

# View current settings
/settings

# Check statistics
/stats
```

## Best Practices

1. **Use /init for new projects** - Creates project context file
2. **Set appropriate approval mode** - Balance convenience and safety
3. **Save important sessions** - Use `/chat save` for complex work
4. **Use @commands for context** - Attach relevant files to queries
5. **Compress when needed** - Save tokens on long conversations
6. **Leverage custom commands** - Create shortcuts for frequent tasks
7. **Review before syncing** - Check changes before accepting them
8. **Use memory for consistency** - Add project-specific guidelines

## Troubleshooting

### "Command not found"
- Type `/help` to see all available commands
- Check spelling of slash commands

### "File not found" with @commands
- Ensure you're using paths relative to project root
- Use `!ls` to check what files exist in sandbox

### "Session not found"
- Use `/chat list` to see available sessions
- Check that the session ID is correct

### "Approval required" blocking workflow
- Change approval mode: `/approval-mode auto-edit` or `/approval-mode yolo`
- Review current mode: `/approval-mode`
