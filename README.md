# Safe Coder

An **AI coding orchestrator** that delegates tasks to specialized AI CLI agents (Claude Code, Gemini CLI) running in isolated git workspaces. Safe Coder handles high-level planning and task decomposition, then coordinates multiple AI agents to execute the work in parallel.

## Features

### 🎯 **Orchestrator Mode (New!)**
- **Multi-Agent Delegation**: Orchestrate Claude Code, Gemini CLI, and other AI agents
- **Task Planning**: Automatically break down complex requests into manageable tasks
- **Workspace Isolation**: Each task runs in its own git worktree/branch
- **Parallel Execution**: Run multiple AI agents concurrently
- **Automatic Merging**: Merge completed work back to main branch

### 🔒 **Security First**
- **Git Worktree Isolation**: Each agent operates in its own git worktree
- **Git Change Tracking**: Every modification automatically tracked with git
- **Safe Merge Back**: Changes reviewed and merged only on completion
- **Rollback Support**: Undo any changes made by agents

### 🎨 **Beautiful Interface**
- **Cyberpunk TUI**: Modern neon-themed terminal UI with pulsing borders and animations
- **Multi-Panel Layout**: Conversation, status, and tool execution panels
- **Real-time Updates**: Live monitoring of agent status
- **Dynamic Processing**: Animated braille spinners and status messages

### 🤖 **AI-Powered Coding**
- **Multiple LLM Providers**: Claude, OpenAI, or Ollama (local models)
- **Privacy Option**: Run 100% locally with Ollama - no API costs, complete privacy
- **Full Tool Suite**: Read, write, edit files, and execute bash commands
- **Contextual Awareness**: Agent understands your codebase and makes intelligent changes

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Safe Coder Orchestrator                       │
│                                                                  │
│  ┌──────────────┐    ┌──────────────────────────────────────┐   │
│  │   Planner    │───►│         Task Queue                   │   │
│  │  (Decompose  │    │  ┌────────┐ ┌────────┐ ┌────────┐   │   │
│  │   requests)  │    │  │ Task 1 │ │ Task 2 │ │ Task 3 │   │   │
│  └──────────────┘    │  └────────┘ └────────┘ └────────┘   │   │
│                      └──────────────────────────────────────┘   │
│                                    │                             │
│         ┌──────────────────────────┼─────────────────────┐      │
│         ▼                          ▼                     ▼      │
│  ┌─────────────┐           ┌─────────────┐        ┌──────────┐ │
│  │ Git Worktree│           │ Git Worktree│        │Git Branch│ │
│  │   Worker 1  │           │   Worker 2  │        │ Worker 3 │ │
│  │ (Claude Code)│          │ (Gemini CLI)│        │(Claude)  │ │
│  └──────┬──────┘           └──────┬──────┘        └────┬─────┘ │
│         │                         │                     │       │
│         └────────────────────┬────┴────────────────────┘       │
│                              ▼                                   │
│                    ┌──────────────────┐                         │
│                    │   Merge Results  │                         │
│                    │  (git merge)     │                         │
│                    └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

1. **External AI CLIs** (at least one):
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `npm install -g @anthropic-ai/claude-code` or via the official installer
   - [Gemini CLI](https://github.com/google/gemini-cli): Install from official repository

2. **Git**: Required for workspace isolation

### Installation

```bash
# Clone the repository
git clone <your-repo-url>
cd safe-coder

# Build the project
cargo build --release

# Install the binary
sudo cp target/release/safe-coder /usr/local/bin/
```

### Usage

#### Orchestrate Mode (Recommended)

```bash
# Interactive orchestration
cd /path/to/your/project
safe-coder orchestrate

# Execute a specific task
safe-coder orchestrate --task "Refactor the auth module and add tests"

# Use a specific worker
safe-coder orchestrate --worker gemini --task "Fix the typo in README.md"

# Disable worktrees (use branches instead)
safe-coder orchestrate --worktrees false
```

#### Direct Chat Mode

```bash
# Start a TUI chat session (direct AI interaction, no delegation)
safe-coder chat

# Classic CLI mode
safe-coder chat --tui false
```

## TUI Orchestration

Within the TUI chat mode, you can spin off background tasks using the `/orchestrate` (or `/orch`) command:

```
/orchestrate Refactor the auth module and add comprehensive tests
/orch Fix the typo in README.md
```

The TUI will:
- Display orchestration progress in the chat panel
- Show worker status in the "WORKERS" panel (right side)
- Track background tasks in the "BACKGROUND TASKS" panel
- Update status in real-time as workers complete

### TUI Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `^C` | Exit the application |
| `/orch <task>` | Orchestrate a task in background |
| `↑↓` | Scroll through messages |
| `Tab` | Switch between panels |

## Orchestrator Commands

When in interactive orchestrate mode:

| Command | Description |
|---------|-------------|
| `exit` / `quit` | End session and cleanup workspaces |
| `status` | Show status of all active workers |
| `cancel` | Cancel all running workers |
| `help` | Show help message |
| *any text* | Submit as a task to orchestrate |

### Example Session

```
🎯 Safe Coder Orchestrator
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Project: /home/user/my-project
Default worker: ClaudeCode
Using worktrees: true

Enter tasks to orchestrate (type 'exit' to quit, 'status' for worker status):

🎯 > Refactor the user service and add comprehensive tests

📋 Planning task: Refactor the user service and add comprehensive tests

Plan to address: "Refactor the user service and add comprehensive tests"

Breaking down into 2 task(s):
  1. Refactor the user service
  2. Add comprehensive tests

📊 Orchestration Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Tasks: 2 total, 2 successful, 0 failed

✓ Task task-1: Refactor the user service
  Worker: ClaudeCode
  Workspace: /project/.safe-coder-workspaces/task-1

✓ Task task-2: Add comprehensive tests
  Worker: ClaudeCode
  Workspace: /project/.safe-coder-workspaces/task-2

🎯 > exit
🧹 Cleaning up workspaces...
✨ Orchestrator session ended. Goodbye!
```

## Configuration

The configuration is stored in `~/.config/safe-coder/config.toml`:

```toml
[llm]
provider = "anthropic"
api_key = "your-api-key-here"
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[git]
auto_commit = true

# Orchestrator configuration
[orchestrator]
claude_cli_path = "claude"     # Path to Claude Code CLI
gemini_cli_path = "gemini"     # Path to Gemini CLI
max_workers = 3                 # Maximum concurrent workers
default_worker = "claude"       # Default: "claude" or "gemini"
use_worktrees = true            # Use git worktrees for isolation
```

## How It Works

### 🎯 **Orchestration Flow**

1. **Request Analysis**: The planner analyzes your request and identifies distinct tasks
2. **Workspace Creation**: Each task gets its own git worktree (isolated copy)
3. **Worker Assignment**: Tasks are assigned to AI agents (Claude Code, Gemini CLI)
4. **Parallel Execution**: Workers execute tasks in their isolated workspaces
5. **Result Merging**: Successful changes are merged back to the main branch
6. **Cleanup**: Temporary worktrees are removed

### 📝 **Task Decomposition**

The planner automatically splits complex requests:

```
Input: "Add authentication, then create user CRUD endpoints, and write tests"

Output:
  Task 1: Add authentication
  Task 2: Create user CRUD endpoints (depends on Task 1)
  Task 3: Write tests (depends on Tasks 1 & 2)
```

### 🔀 **Git Worktree Isolation**

```
project/
├── .git/                          # Main repository
├── .safe-coder-workspaces/        # Worktree base
│   ├── task-1/                    # Isolated workspace for task 1
│   │   ├── src/
│   │   └── ... (full project copy)
│   └── task-2/                    # Isolated workspace for task 2
│       ├── src/
│       └── ...
└── src/                           # Main project files
```

## Development

### Project Structure

```
safe-coder/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── config.rs            # Configuration management
│   ├── orchestrator/        # NEW: Orchestration module
│   │   ├── mod.rs           # Orchestrator coordinator
│   │   ├── planner.rs       # Task decomposition
│   │   ├── worker.rs        # CLI worker management
│   │   ├── workspace.rs     # Git worktree manager
│   │   └── task.rs          # Task definitions
│   ├── llm/                 # LLM client integrations
│   ├── tools/               # Agent tools
│   ├── tui/                 # Terminal UI
│   ├── git/                 # Git change tracking
│   └── session/             # Session management
├── Cargo.toml
└── README.md
```

### Building

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Check for errors
cargo check
```

## Troubleshooting

### CLI Not Found

```
Error: Claude Code CLI not found at 'claude'
```

**Solution**: Install the CLI or update the path in config:
```bash
# Install Claude Code
npm install -g @anthropic-ai/claude-code

# Or update config
safe-coder config --show  # Then edit ~/.config/safe-coder/config.toml
```

### Worktree Issues

```
Error: Failed to create worktree
```

**Solution**: Ensure you're in a git repository:
```bash
git init  # If not already a git repo
```

### Merge Conflicts

When tasks modify the same files, you may encounter merge conflicts:
```
Error: Merge conflict when integrating task-2. Manual resolution needed.
```

**Solution**: Resolve conflicts manually in the main repository.

## Future Enhancements

- [x] Orchestrator with multi-agent delegation
- [x] Git worktree isolation for tasks
- [x] Automatic task decomposition
- [x] Parallel worker execution
- [ ] LLM-assisted task planning (using AI for smarter decomposition)
- [ ] Dependency-aware task scheduling
- [ ] Interactive conflict resolution in TUI
- [ ] Custom worker plugins
- [ ] Task progress visualization
- [ ] Checkpoint and resume for long tasks

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - See LICENSE file for details

## Acknowledgments

- Orchestrates [Claude Code](https://docs.anthropic.com/en/docs/claude-code) and [Gemini CLI](https://github.com/google/gemini-cli)
- TUI powered by [Ratatui](https://github.com/ratatui-org/ratatui)
- Built with Rust for performance and safety
