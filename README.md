# Safe Coder

A powerful **AI coding CLI** and **multi-agent orchestrator** built in Rust. Safe Coder works as a standalone coding assistant with full tool capabilities, and can also delegate complex tasks to specialized AI CLI agents (Claude Code, Gemini CLI) running in isolated git workspaces.

![Safe Coder CLI](assets/cli-screenshot.png)

## Features

### 🖥️ **Interactive Shell Mode (New Warp-like TUI)**
- **Command Block Interface**: Modern shell with visual command blocks (like Warp terminal)
- **AI Integration**: Use `@connect` and `@ <query>` for inline AI assistance
- **Real-time Tool Display**: See AI tool calls execute in real-time (like Claude Code)
- **Diff Rendering**: File edits show compact diffs with +/- indicators for changes
- **Smart Autocomplete**: Tab completion for commands and file paths with popup UI
- **Scrolling Support**: Mouse scroll wheel and Shift+Up/Down for navigation
- **Streaming Output**: Real-time command feedback with bordered output blocks
- **Context-Aware AI**: AI queries include shell context (last 10 commands + outputs)
- **Full Shell Features**: cd, pwd, history, export, env, and all standard commands

### 💻 **Standalone Coding CLI**
- **Direct AI Coding**: Use Safe Coder as your coding assistant without external CLIs
- **Full Tool Suite**: Read, write, edit files, and execute bash commands
- **Multiple LLM Providers**: Claude, OpenAI, or Ollama (local models)
- **Privacy Option**: Run 100% locally with Ollama - no API costs, complete privacy
- **Beautiful TUI**: Modern terminal UI with syntax highlighting

### 🎯 **Orchestrator Mode**
- **Multi-Agent Delegation**: Orchestrate Claude Code, Gemini CLI, and other AI agents
- **Task Planning**: Automatically break down complex requests into manageable tasks
- **Workspace Isolation**: Each task runs in its own git worktree/branch
- **Parallel Execution**: Run up to 3 AI agents concurrently with intelligent throttling
- **Throttle Control**: Per-worker-type concurrency limits and start delays to respect rate limits
- **Automatic Merging**: Merge completed work back to main branch

### 🔒 **Security First**
- **Git Worktree Isolation**: Each agent operates in its own git worktree
- **Git Change Tracking**: Every modification automatically tracked with git
- **Safe Merge Back**: Changes reviewed and merged only on completion
- **Rollback Support**: Undo any changes made by agents

### 🎨 **Beautiful Interface**
- **Shell-First TUI**: Modern Warp-like terminal with command blocks and bordered output
- **Real-time Tool Execution**: Watch AI tools execute live with progress indicators and status
- **Smart Diff Display**: File edits show compact diffs with +/- indicators for easy review
- **Smart Autocomplete**: Tab completion popup with command and path suggestions  
- **Scrolling Navigation**: Mouse wheel and keyboard shortcuts for smooth scrolling
- **Cyberpunk Chat TUI**: Neon-themed terminal UI with pulsing borders and animations
- **Multi-Panel Layout**: Conversation, status, and tool execution panels
- **Real-time Updates**: Live monitoring of agent status
- **Dynamic Processing**: Animated braille spinners and status messages

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

1. **For Orchestrator Mode** (optional - at least one external CLI):
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `npm install -g @anthropic-ai/claude-code`
   - [Gemini CLI](https://github.com/google/gemini-cli): Install from official repository

2. **Git**: Required for workspace isolation and change tracking

3. **API Key**: For Claude, OpenAI, or run locally with Ollama

### Installation

#### From GitHub Releases (Recommended)

Download the latest release for your platform from the [Releases page](https://github.com/siddharth-ghatti/Safe-Coder/releases).

**Linux / macOS:**
```bash
# Download the binary for your platform (choose one)

# For Linux x86_64:
curl -LO https://github.com/siddharth-ghatti/Safe-Coder/releases/latest/download/safe-coder-linux-x86_64
chmod +x safe-coder-linux-x86_64
sudo mv safe-coder-linux-x86_64 /usr/local/bin/safe-coder

# For macOS Intel:
curl -LO https://github.com/siddharth-ghatti/Safe-Coder/releases/latest/download/safe-coder-macos-x86_64
chmod +x safe-coder-macos-x86_64
sudo mv safe-coder-macos-x86_64 /usr/local/bin/safe-coder

# For macOS Apple Silicon (M1/M2/M3):
curl -LO https://github.com/siddharth-ghatti/Safe-Coder/releases/latest/download/safe-coder-macos-aarch64
chmod +x safe-coder-macos-aarch64
sudo mv safe-coder-macos-aarch64 /usr/local/bin/safe-coder
```

**Windows:**
Download `safe-coder-windows-x86_64.exe` from the releases page and either run it directly or add it to your PATH.

#### From Source

```bash
# Clone the repository
git clone https://github.com/siddharth-ghatti/Safe-Coder.git
cd Safe-Coder

# Build the project
cargo build --release

# Install the binary
sudo cp target/release/safe-coder /usr/local/bin/
```

## Usage

Safe Coder offers multiple modes to fit your workflow:

### Shell Mode

Start the modern shell-first TUI with Warp-like command blocks:

```bash
# Start the new shell TUI (default)
safe-coder shell

# Start the shell TUI in a specific directory
safe-coder shell --path /path/to/project

# Use legacy text-based shell (no TUI)
safe-coder shell --no-tui
```

**New Shell TUI Interface:**
- **Command Blocks**: Each command gets its own visual block with bordered output
- **Real-time Streaming**: See command output as it happens
- **Real-time Tool Execution**: AI tool calls appear and execute live (like Claude Code)
- **File Edit Diffs**: See exactly what changed with compact +/- diff display
- **Smart Autocomplete**: Tab key cycles through command and path suggestions
- **Scrolling**: Use mouse wheel or Shift+Up/Down to navigate history

**AI Commands (in shell TUI):**

| Command | Description |
|---------|-------------|
| `@connect` | Connect to AI for coding assistance |
| `@ <question>` | Ask AI for help (includes shell context automatically) |
| `@orchestrate <task>` | Delegate task to background AI agents |

**Navigation & Controls:**

| Key | Action |
|-----|--------|
| `Tab` | Cycle through autocomplete suggestions |
| `Shift+Tab` | Cycle backwards through suggestions |
| `Enter` or `→` | Apply selected autocomplete suggestion |
| `Shift+↑/↓` | Scroll through command history |
| `Mouse Wheel` | Scroll up/down through output |
| `PageUp/PageDown` | Fast scroll through output |
| `↑/↓` | Navigate command history |
| `^C` | Exit the shell |

### Chat Mode (Direct AI Coding)

```bash
# Start a TUI chat session
safe-coder chat

# Classic CLI mode (no TUI)
safe-coder chat --tui false

# Use plan mode (requires approval before tool execution)
safe-coder chat --mode plan
```

### Orchestrate Mode (Multi-Agent)

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

## Example Sessions

### New Shell TUI Mode

```
┌─────────────────────────────────────────────────────────────┐
│  Safe Coder Shell - Modern TUI with command blocks         │
└─────────────────────────────────────────────────────────────┘

┌─ Command Block 1 ───────────────────────────────────────────┐
│ my-project (main) $ ls -la                                  │
│ ┌─ Output ─────────────────────────────────────────────────┐ │
│ │ total 24                                                 │ │
│ │ drwxr-xr-x  8 user  staff   256 Dec 26 10:00 .          │ │
│ │ -rw-r--r--  1 user  staff  1234 Dec 26 10:00 Cargo.toml │ │
│ │ drwxr-xr-x  5 user  staff   160 Dec 26 10:00 src        │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘

┌─ Command Block 2 ───────────────────────────────────────────┐
│ my-project (main) $ @connect                                │
│ ┌─ Output ─────────────────────────────────────────────────┐ │
│ │ ✓ Connected to AI. Use '@ <question>' for assistance.    │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘

┌─ Command Block 3 ───────────────────────────────────────────┐
│ 🤖 my-project (main) $ @ how do I add a new dependency?     │
│ ┌─ AI Response ────────────────────────────────────────────┐ │
│ │ 🤖 Thinking...                                          │ │
│ │ Based on your Cargo.toml, you can add dependencies by:  │ │
│ │                                                          │ │
│ │ ✓ Tool: edit_file                                       │ │
│ │ ┌─ File Diff: Cargo.toml ─┐                            │ │
│ │ │ @@ -8,6 +8,7 @@           │                            │ │
│ │ │  [dependencies]          │                            │ │
│ │ │  tokio = "1.0"          │                            │ │
│ │ │  serde = "1.0"          │                            │ │
│ │ │ +clap = "4.0"           │                            │ │
│ │ │  [dev-dependencies]      │                            │ │
│ │ │  test = "0.1"           │                            │ │
│ │ └──────────────────────────┘                            │ │
│ │                                                          │ │
│ │ I've added the clap dependency to your Cargo.toml!      │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘

┌─ Input ─────────────────────────────────────────────────────┐
│ my-project (main) $ cargo add ser[TAB]                      │
│ ┌─ Autocomplete ─┐                                          │
│ │ > serde        │                                          │ │
│ │   serde_json   │                                          │ │
│ │   serialize    │                                          │ │
│ └────────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
```

### Legacy Shell Mode

```
┌─────────────────────────────────────────────────────────────┐
│  Safe Coder Shell - Legacy text-based shell (--no-tui)     │
└─────────────────────────────────────────────────────────────┘

my-project (main) ❯ ls -la
total 24
drwxr-xr-x  8 user  staff   256 Dec 26 10:00 .
-rw-r--r--  1 user  staff  1234 Dec 26 10:00 Cargo.toml
drwxr-xr-x  5 user  staff   160 Dec 26 10:00 src

my-project (main) ❯ ai-connect
Connecting to AI...
✓ Connected to AI. Use 'ai <question>' for assistance.

🤖 my-project (main) ❯ ai how do I add a new dependency to Cargo.toml?
🤖 Thinking...

To add a new dependency to Cargo.toml, you can either:

1. Manually edit Cargo.toml and add under [dependencies]:
   [dependencies]
   serde = "1.0"

2. Use cargo add (requires cargo-edit):
   cargo add serde

🤖 my-project (main) ❯ chat

━━━ Entering Chat Mode ━━━
Type your requests for AI coding assistance.
Type 'exit' or 'shell' to return to shell mode.

chat> Add serde with derive feature to my project
🤖 Processing...

I'll add serde with the derive feature to your Cargo.toml.

🔧 Executing 1 tool(s): edit_file

Done! I've added `serde = { version = "1.0", features = ["derive"] }` to your dependencies.

chat> shell

━━━ Returning to Shell Mode ━━━

🤖 my-project (main) ❯ exit
Goodbye!
```

### Orchestrator Mode

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

**Shell TUI Mode:**

| Key | Action |
|-----|--------|
| `^C` | Exit the shell |
| `Tab` | Cycle through autocomplete suggestions |
| `Shift+Tab` | Cycle backwards through autocomplete suggestions |
| `Enter` or `→` | Apply selected autocomplete suggestion |
| `Shift+↑/↓` | Scroll through command history/output |
| `Mouse Wheel` | Scroll up/down through output |
| `PageUp/PageDown` | Fast scroll through output |
| `↑/↓` | Navigate command history |

**Chat TUI Mode:**

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

# Tool settings
[tools]
bash_timeout_secs = 120
max_output_bytes = 1048576
warn_dangerous_commands = true

# Orchestrator configuration
[orchestrator]
claude_cli_path = "claude"      # Path to Claude Code CLI
gemini_cli_path = "gemini"      # Path to Gemini CLI
max_workers = 3                 # Maximum concurrent workers (up to 3)
default_worker = "claude"       # Default: "claude" or "gemini"
use_worktrees = true            # Use git worktrees for isolation

# Throttle limits for controlling worker concurrency by type
[orchestrator.throttle_limits]
claude_max_concurrent = 2       # Max concurrent Claude workers
gemini_max_concurrent = 2       # Max concurrent Gemini workers
start_delay_ms = 100            # Delay between starting workers (ms)
```

## How It Works

### 💻 **Direct Coding Mode**

Safe Coder functions as a complete AI coding assistant:

1. **Tool Execution**: The AI can read, write, and edit files, plus run bash commands
2. **Git Tracking**: All changes are automatically committed with descriptive messages
3. **Approval Modes**: 
   - **Act Mode** (default): AI executes tools automatically
   - **Plan Mode**: Shows execution plan and asks for approval first

### 🖥️ **Shell Mode**

The shell now features a modern Warp-like TUI interface with enhanced functionality:

1. **Command Blocks**: Each command execution is visually contained in its own block
2. **Smart Autocomplete**: Tab completion for commands and file paths with visual popup
3. **AI Integration**: Use `@connect` and `@ <query>` for context-aware AI assistance  
4. **Real-time Tool Execution**: Watch AI tools execute live with progress indicators and checkmarks
5. **Diff Rendering**: File edits show compact diffs with +/- indicators for easy change review
6. **Scrolling Navigation**: Mouse wheel and keyboard shortcuts for smooth navigation
7. **Real-time Output**: Streaming command output with bordered visual containers
8. **Context Awareness**: AI queries automatically include shell context (recent commands and outputs)
9. **Git Auto-commit Control**: Shell mode disables git auto-commit to prevent unwanted repository changes

### 🎯 **Orchestration Flow**

1. **Request Analysis**: The planner analyzes your request and identifies distinct tasks
2. **Workspace Creation**: Each task gets its own git worktree (isolated copy)
3. **Worker Assignment**: Tasks are assigned to AI agents (Claude Code, Gemini CLI)
4. **Parallel Execution**: Workers execute tasks concurrently (up to 3 at once)
5. **Result Merging**: Successful changes are merged back to the main branch
6. **Cleanup**: Temporary worktrees are removed

### ⚡ **Parallel Execution with Throttling**

Safe Coder can run up to 3 CLI agents in parallel, with intelligent throttling:

- **Global Concurrency Limit**: Maximum of 3 workers running simultaneously
- **Per-Worker-Type Limits**: Control how many Claude or Gemini workers can run at once
- **Start Delay**: Configurable delay between starting workers

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
│   └── task-2/                    # Isolated workspace for task 2
└── src/                           # Main project files
```

## Development

### Project Structure

```
safe-coder/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── config.rs            # Configuration management
│   ├── shell/               # Shell mode module
│   │   └── mod.rs           # Interactive shell with AI
│   ├── orchestrator/        # Orchestration module
│   │   ├── mod.rs           # Orchestrator coordinator
│   │   ├── planner.rs       # Task decomposition
│   │   ├── worker.rs        # CLI worker management
│   │   ├── workspace.rs     # Git worktree manager
│   │   └── task.rs          # Task definitions
│   ├── session/             # Chat session management
│   ├── llm/                 # LLM client integrations
│   ├── tools/               # Agent tools (read, write, edit, bash)
│   ├── tui/                 # Terminal UI
│   └── git/                 # Git change tracking
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

### API Key Issues

```
Error: Failed to create LLM client
```

**Solution**: Configure your API key:
```bash
# Set via config command
safe-coder config --api-key YOUR_API_KEY

# Or login with OAuth
safe-coder login anthropic
```

## Future Enhancements

- [x] Orchestrator with multi-agent delegation
- [x] Git worktree isolation for tasks
- [x] Automatic task decomposition
- [x] Parallel worker execution
- [x] Interactive shell mode with AI
- [x] Standalone coding CLI
- [x] Modern Warp-like shell TUI with command blocks
- [x] Smart autocomplete with Tab completion
- [x] Scrolling support (mouse wheel + keyboard shortcuts)
- [x] Context-aware AI integration in shell mode
- [x] Git auto-commit control for shell mode
- [x] Real-time tool call display with diff rendering
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
- Diff rendering powered by the [Similar](https://github.com/mitsuhiko/similar) crate
- Built with Rust for performance and safety
