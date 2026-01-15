# Safe Coder

**Safe Coder** is a Rust-powered, AI-first shell and multi-agent orchestrator for lightning-fast, safe, and powerful large-scale code automation. It combines a modern terminal interface (TUI), deep project awareness, interactive planning, and an innovative multi-agent system—delivering the world's safest and most productive way to use AI for real codebases.

---

## 🚀 What's New

- **PLAN vs BUILD Agent Modes**—Visual mode switcher in the TUI sidebar, `/agent` slash command, and `Ctrl+G` keyboard shortcut. Agent mode is always color-coded (GREEN = BUILD, CYAN = PLAN), with session synchronization for clarity and safety boundaries.
- **Visual Steps/Tasks Sidebar**—Sidebar displays plans/steps with real-time color-coded status: completed (green), running (cyan, animated), failed (red). Tracks both planning and execution phases; details in AGENT_MODE_SUMMARY.md and STEPS_VISUAL_GUIDE.md.
- **Improved Multi-Agent Orchestration**—Parallelizes up to 3 agents, supports dependency-aware decomposition, and merges code across isolated worktrees.
- **Expanded UI/UX**—Modern, themeable TUI based on Ratatui with rich context panes, context-aware autocomplete, file pickers, and more.
- **Direct LSP (Language Server Protocol) Support**—Auto-managed language servers, inline diagnostic/error highlighting, automatic LSP downloads and configuration, and code intelligence for multiple languages.
- **Desktop App & HTTP Server**—Built-in HTTP/WebSocket server (default: 127.0.0.1:9876) for Tauri-based desktop integration and third-party clients; includes REST APIs, SSE events, and a PTY WebSocket for terminal access.
- **Skill System, Hooks, and Fine-Grained Permissions**—Isolate knowledge, enforce workflow policies, and control exactly how/when AI touches your code.

---

## Demo

<p align="center">
  <img src="assets/safe-coder-demo.gif" alt="Safe Coder Demo" width="800">
</p>

---

## Table of Contents

- [Why Safe Coder?](#why-safe-coder)
- [Demo: Shell, Planning, Orchestration](#demo)
- [Quick Start](#quick-start)
- [Interactive TUI Features](#interactive-tui-features)
- [Agent/Planning Modes](#agentplanning-modes)
- [Advanced Orchestration & Subagents](#advanced-orchestration--subagents)
- [Desktop App & HTTP Server](#desktop-app--http-server)
- [Configuration, Customization, and Safety](#configuration-customization-and-safety)
- [Project Structure](#project-structure)
- [Contributing & License](#contributing--license)

---

## Why Safe Coder?

| Feature | Safe Coder | Claude Code | Cursor | Aider |
|---------|------------|-------------|--------|-------|
| **Visual AI Shell** | ✅ | ❌ | ❌ | ❌ |
| **Multi-Agent Orchestration** | ✅ | ❌ | ❌ | ❌ |
| **Semantic Planning (PLAN/BUILD)** | ✅ | ❌ | ❌ | ❌ |
| **Real Steps/Progress UI** | ✅ | ❌ | ❌ | ❌ |
| **LSP Integration/IDE Features** | ✅ | ❌ | Built-in | ❌ |
| **Hooks/Skills/Custom Safety** | ✅ | ❌ | ❌ | ❌ |
| **AST-based Code Search** | ✅ | ❌ | ❌ | ❌ |
| **Subagent System** | ✅ | ❌ | ❌ | ❌ |
| **Isolation/Checkpoints** | ✅ | ❌ | ❌ | ❌ |
| **Undo/Redo** | ✅ | ✅ | ❌ | ❌ |
| **Local AI (Ollama)** | ✅ | ❌ | ❌ | ✅ |
| **75+ Model Support (OpenRouter)** | ✅ | ❌ | Limited | ✅ |
| **Native (Rust, Fast)** | ✅ | Node.js | Electron | Python |

---


---

## Interactive TUI Features

- **Shell Mode/TUI**:  
  Modern command blocks, clickable file pickers, live AI panel, and stepwise task display.
- **Agent Mode**:  
  Toggle PLAN (read-only/planning) vs BUILD (full edit/act) visually. Shows in a sidebar, `/agent` toggles, or `Ctrl+G`.
- **Stepwise Progress**:  
  Live sidebar shows planning, running tools, dependencies, and current state (color-coded).
- **Autocomplete & Context**:  
  Tab-complete tools, files, and custom commands; @-attach files; viewing/editing in-place.
- **LSP-powered Code Intelligence**:  
  Inline error highlighting, diagnostics, and completions for Rust, JS/TS, Python, Go, and more.

---

## Advanced Orchestration & Subagents

- **Decompose Requests**:  
  Free-form natural language ("Add auth then write tests") → task graph, with dependencies.
- **Isolated Execution**:  
  Each AI worker runs in a separate git worktree or branch. Fully parallelized (up to 3 agents).
- **Subagents**:  
  Specialized roles: Analyzer (read-only/code audit), Tester, Refactorer, Documenter, Custom.
    - Per-agent LLM config (Claude, GPT, Ollama, etc.)
    - Toolset restrictions for safety
    - Real-time progress monitoring
- **Multi-Agent Throttling & Strategy**:  
  Assign tasks round-robin, prefer fastest/cheapest agent, or follow dependency order.

---

## Project Structure

```
safe-coder/
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── shell/                # Shell/TUI logic
│   ├── orchestrator/         # Planning + agent orchestration
│   ├── tui/                  # Terminal UI
│   ├── subagents/            # Analyzer/Tester/etc.
│   ├── llm/                  # Language model clients
│   ├── tools/                # File/batch/bash/grep/ast tools
│   ├── skills/               # Context injection
│   ├── hooks/                # Safety/lifecycle hooks
│   └── ...
├── Cargo.toml
└── README.md
```

---

## Contributing & License

We welcome contributions! PRs, feature requests, or bug reports are encouraged.

**MIT License** (see LICENSE for details).

---

## Credits

- Powered by [Ratatui](https://github.com/ratatui-org/ratatui), [Tree-sitter](https://tree-sitter.github.io/tree-sitter/), [Similar](https://github.com/mitsuhiko/similar)
- Multi-agent logic inspired by best practices from AI research and large engineering teams.

---

**Safe Coder:** Warp-speed AI coding with end-to-end transparency, fine-grained safety, and unmatched ergonomics.

<p align="center">
  <img src="assets/orchestration-demo.gif" alt="Multi-Agent Orchestration" width="800">
</p>

## Quick Start (30 seconds)

```bash
# Install (macOS/Linux)
curl -LO https://github.com/siddharth-ghatti/Safe-Coder/releases/latest/download/safe-coder-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)
chmod +x safe-coder-* && sudo mv safe-coder-* /usr/local/bin/safe-coder

# Set your API key (choose one)
export OPENROUTER_API_KEY="sk-or-v1-..."  # 75+ models
# OR
export ANTHROPIC_API_KEY="sk-ant-..."     # Claude directly

# Start the interactive shell
cd your-project
safe-coder                  # Starts interactive shell
safe-coder --ai             # Starts with AI connected

# Start the HTTP server for desktop integration (optional)
# Default: 127.0.0.1:9876
safe-coder serve
```

**Key Commands in Shell:**
```bash
# Shell commands work normally
ls -la
git status
cargo test

# AI assistance commands  
ai-connect                   # Connect to AI assistant
ai how do I add auth?        # Ask AI for help with shell context
chat                         # Enter interactive coding mode

# In chat mode
@file.rs                     # Add file to context
!cargo test                  # Run shell command without leaving chat
/undo                        # Undo last changes
/redo                        # Redo changes
/compact                     # Free up context tokens

# Multi-agent orchestration
orchestrate add auth and tests  # Delegate complex tasks
```

## Shell-First Workflow Guide

Safe Coder's shell is designed as the primary interface, combining the familiarity of a terminal with powerful AI assistance. Here's how to make the most of it:

### Getting Started with the Shell

```bash
# Start the interactive shell
cd your-project
safe-coder                    # Default shell mode

# Connect to AI assistance
ai-connect                    # Within the shell
# OR start with AI connected
safe-coder --ai              # From command line
```

### Core Shell Features

**🖥️ All your regular commands work:**
```bash
ls -la
git status
npm test
cargo build
python main.py
```

**🤖 AI assistance when you need it:**
```bash
# Ask for help with current shell context
ai how do I optimize this Rust code?
ai what's wrong with my git setup?
ai suggest a better project structure

# Get specific coding help
ai write a dockerfile for this node app
ai fix the failing tests
```

**🔄 Seamless mode switching:**
```bash
# Enter full coding mode when needed
chat
  # Now in chat mode - full AI conversation
  # Use @file.rs to add context
  # Use !command to run shell commands
  # Type 'exit' to return to shell

# Back to shell mode for regular commands
exit  # or 'shell'
```

### Advanced Shell Workflows

**📁 Project Discovery:**
```bash
ls                           # Browse files
find . -name "*.rs"         # Search for files
ai analyze this codebase    # Get AI overview
```

**🔍 Debugging Sessions:**
```bash
cargo test                  # Run tests
# Test fails...
ai explain this error       # Get AI help with error context
# AI suggests fix
cargo test                  # Try again
```

**🚀 Development Flow:**
```bash
git status                  # Check repo state
ai review my recent changes # AI reviews your work
git add .
git commit -m "$(ai suggest a commit message)"
```

**🛠️ Multi-Step Tasks:**
```bash
# Complex tasks that need multiple steps
ai-connect
chat
# Now in full AI mode
@src/**/*.rs               # Add all source files to context
refactor the auth module to use async/await
# AI does the refactoring with full context
exit
# Back to shell
cargo test                 # Verify changes
```

### Shell vs Chat vs Orchestrate

| Mode | Best For | Example Use |
|------|----------|-------------|
| **Shell** | Daily development, quick help | `git status`, `ai fix this error` |
| **Chat** | Focused coding tasks | Adding features, refactoring code |
| **Orchestrate** | Complex multi-step projects | `orchestrate "build a REST API with auth"` |

### Pro Tips

1. **Use shell for quick questions**: Instead of leaving your terminal, just ask `ai how do I...`

2. **Chat mode for deep work**: When you need AI to understand lots of context and make multiple changes

3. **Orchestration for big tasks**: Let AI agents work in parallel on different parts of large projects

4. **Mix and match**: Start in shell, go to chat for focused work, return to shell for testing

## 🌟 What's New

### 🚀 **Advanced Extensibility Features (v2.6)**

#### 🤖 **Multi-Model Subagents**
Configure different LLM providers per subagent type for optimal cost/performance:
```toml
[subagents.analyzer]
provider = "anthropic"
model = "claude-sonnet-4-20250514"  # Best for analysis

[subagents.tester]
provider = "openai"
model = "gpt-4o"  # Fast for test generation

[subagents.documenter]
provider = "ollama"
model = "llama3.1:8b"  # Local, free for docs
```

#### 🔍 **AST-Grep (Structural Code Search)**
Search code by structure, not just text. Uses tree-sitter for AST parsing:
```bash
# Find all function definitions in Rust
safe-coder ast-grep --pattern "function_item" --language rust

# Find all Python classes
safe-coder ast-grep --pattern "class_definition" --language python

# Use tree-sitter queries for complex patterns
safe-coder ast-grep --pattern "(function_item name: (identifier) @name)"
```
Supports: **Rust, TypeScript, JavaScript, Python, Go**

#### 📚 **Skill System**
Load specialized knowledge into AI context via markdown files:
```bash
# List available skills
/skill list

# Activate a skill
/skill activate rust-patterns

# Skills auto-activate based on file patterns
# Working on *.rs files? rust-patterns activates automatically!
```

Create custom skills in `.safe-coder/skills/`:
```markdown
---
name: my-api-patterns
trigger: ["*.ts", "src/api/**"]
description: Our API conventions
---

# API Patterns

When creating API endpoints:
1. Always use zod for validation
2. Return consistent error shapes
...
```

**Built-in Skills:**
- `rust-patterns` - Rust idioms (triggers: `*.rs`)
- `react-patterns` - React/TypeScript best practices (triggers: `*.tsx`, `*.jsx`)
- `python-patterns` - Python idioms (triggers: `*.py`)

#### 🪝 **Hooks System**
Add custom logic at lifecycle points for validation, logging, or automation:

**Available Hook Points:**
| Hook | When It Fires |
|------|---------------|
| `PreToolUse` | Before any tool executes |
| `PostToolUse` | After any tool completes |
| `PreFileWrite` | Before writing to a file |
| `PostFileWrite` | After writing to a file |
| `PreBash` | Before bash command execution |
| `PostBash` | After bash command completes |
| `OnError` | When an error occurs |
| `OnContextLimit` | When context limit is approaching |
| `OnSessionStart` | When a new session begins |

**Built-in Hooks:**
- `CommentChecker` - Warns about TODO/FIXME in new code
- `ContextMonitor` - Alerts when context usage is high
- `TodoEnforcer` - Ensures todos are tracked properly
- `EditValidator` - Validates file edits before applying

### 🌐 **OpenRouter + 75+ Models (v2.5)**
- **One API, 75+ models** - Access Claude, GPT-4, Gemini, Llama, Mistral, DeepSeek, and more through a single API
- **Automatic fallback** - If your preferred model is unavailable, OpenRouter routes to a similar one
- **Cost tracking** - See per-model pricing in the OpenRouter dashboard
- **Just set `OPENROUTER_API_KEY`** - Works out of the box

### ⚡ **New Commands (v2.5)**
- **`/undo`** - Instantly undo your last file changes (git-based)
- **`/redo`** - Redo previously undone changes
- **`/compact`** - Manually trigger context compaction to free up tokens
- **`/sessions`** - Quick alias for `/chat list` to switch sessions
- **`/agent`** - Alias for `/mode` to switch between plan/build modes

### 🛡️ **Git-Agnostic Checkpoints (v2.5)**
- **Works without git** - Create checkpoints in any directory
- **Automatic gitignore** - Checkpoint folders auto-added to .gitignore for git projects
- **Fast restore** - Instantly rollback to any previous checkpoint
- **Configurable limits** - Control max checkpoints and ignore patterns

### 🤖 **Subagent System (v2.4)**
- **Specialized AI agents** - Deploy focused agents for specific tasks (analysis, testing, refactoring, documentation)
- **Tool-restricted agents** - Each subagent type has carefully curated tool access for safety
- **Autonomous execution** - Subagents work independently with their own reasoning loops
- **Real-time monitoring** - Track subagent progress with live status updates
- **Smart delegation** - Automatically choose the right subagent type for complex tasks
- **Five subagent types**:
  - 🔍 **Code Analyzer** - Read-only code analysis and insights
  - 🧪 **Tester** - Create and run comprehensive test suites
  - 🔧 **Refactorer** - Make targeted code improvements
  - 📝 **Documenter** - Generate and update documentation
  - 🤖 **Custom Agent** - User-defined specialized roles

### 🧠 **Enhanced Planning (v2.4)**
- **Complexity scoring** - Automatic assessment of task difficulty and scope
- **Intelligent agent assignment** - Match tasks to the most appropriate subagent type
- **Multi-step planning** - Break down complex requests into manageable subagent tasks
- **Dependency tracking** - Ensure proper execution order for dependent steps
- **Progress visualization** - Real-time status updates across all active subagents

### 🧠 **LSP Integration (v2.3)**
- **Language Server Protocol support** - Get IDE-like features directly in the terminal
- **Automatic LSP downloads** - Automatically install and configure language servers
- **Real-time code intelligence** - Syntax highlighting, error detection, and code completion
- **Multi-language support** - Works with Rust, TypeScript, Python, Go, and more
- **Shell-integrated LSP** - Access language features seamlessly in the TUI

### 🚀 **Orchestration Integration (v2.2)**
- **Shell-integrated orchestration** - Run `@orchestrate <task>` directly from the shell TUI
- **GitHub Copilot support** - New worker type using `gh copilot` for task execution
- **Worker distribution strategies** - Single, round-robin, task-based, or load-balanced task distribution
- **Self-orchestration** - Safe-Coder can now use itself as a worker for recursive delegation
- **Plan vs Act modes** - Choose between approval-required planning or auto-execution

### 🧠 **Smarter AI (v2.1)**
- **Context-aware reasoning** - AI understands project structure and conventions
- **Loop detection** - Prevents AI from getting stuck in repetitive patterns
- **Inline bash streaming** - See command output in real-time as it executes
- **Better word wrapping** - Improved text rendering in the TUI

### 🛠️ **Expanded Tool Suite**
- **AST-Grep** - Structural code search using tree-sitter AST parsing
- **Glob search** - Fast file pattern matching with `**/*.rs` syntax
- **Grep search** - Content search across files with regex support
- **File listing** - Directory exploration with smart filtering
- **Todo tracking** - Built-in task management for complex workflows
- **Web fetch** - Retrieve and analyze web content

### 📁 **File Picker UI**
- **Visual file selection** - Browse and select files with a popup interface
- **Keyboard navigation** - Use arrow keys to navigate directories
- **Glob pattern support** - Filter files with patterns like `*.ts`

### 🔐 **Permission Modes**
- **Plan mode** - Preview all actions before execution
- **Default mode** - Ask before each tool call (recommended)
- **Auto-edit mode** - Auto-approve file operations only
- **YOLO mode** - Auto-approve everything (use with caution)

### ⚡ **Simplified Architecture (v2.0)**
- **20x faster startup** - Removed VM/Docker complexity for direct filesystem access
- **Git-based safety** - All changes tracked with automatic commits and easy rollback
- **Cross-platform** - Works seamlessly on Linux, macOS, and Windows
- **1,200+ lines removed** - Cleaner codebase focused on core features

### 🦙 **Local AI Support (Ollama)**
- **100% Private** - Run completely locally with no API costs
- **Offline capable** - Works without internet connection
- **Multiple models** - DeepSeek Coder, Qwen Coder, CodeLlama, and more
- **GPU acceleration** - Automatic NVIDIA/Apple Silicon support

### 🎨 **Enhanced TUI Experience**
- **OpenCode-inspired theme** - Modern VS Code-style interface design
- **Dynamic ASCII banner** with neon gradient effects
- **Cyberpunk theme** - Pulsing neon borders and glitch effects
- **Professional dark mode** - Google CLI / Claude Code inspired styling
- **Animated processing** - Braille spinners and real-time status updates
- **Inline reasoning display** - See AI thought process between tool calls

### ⚡ **Qwen Code CLI Features**
- **Slash commands** (`/help`, `/stats`, `/chat save`) for meta-control
- **At-commands** (`@file.rs`) for context attachment with glob patterns
- **Shell passthrough** (`!cargo test`) for direct command execution
- **Session management** - Save, resume, and delete conversations
- **Approval modes** - Fine-grained control over AI tool execution
- **Custom commands** - User-defined shortcuts for frequent operations

## Features

### 🔍 **AST-Grep (Structural Code Search)**
- **Tree-sitter Powered**: Search code by AST structure, not just text patterns
- **Multi-Language**: Supports Rust, TypeScript, JavaScript, Python, and Go
- **Pattern Types**: Use simple node types (`function_item`) or full tree-sitter queries
- **IDE Integration**: Available as a tool for AI agents to use during coding
- **Fast Indexing**: Respects `.gitignore` and handles large codebases efficiently

### 📚 **Skill System**
- **Knowledge Injection**: Load specialized context into AI conversations
- **Auto-Activation**: Skills activate automatically based on file patterns
- **Custom Skills**: Create project-specific skills in `.safe-coder/skills/`
- **YAML Frontmatter**: Define triggers, descriptions, and metadata
- **Built-in Skills**: Comes with Rust, React, and Python best practices

### 🪝 **Hooks System**
- **Lifecycle Events**: Hook into 9 different lifecycle points
- **Built-in Validators**: Comment checking, context monitoring, edit validation
- **Custom Hooks**: Create your own hooks for project-specific workflows
- **Pre/Post Actions**: Run logic before or after tool execution
- **Error Handling**: Custom error handlers and recovery logic

### 🤖 **Multi-Model Subagents**
- **Per-Agent Configuration**: Different LLM providers per subagent type
- **Cost Optimization**: Use cheaper models for simple tasks
- **Provider Flexibility**: Mix Anthropic, OpenAI, OpenRouter, and Ollama
- **Automatic Selection**: Falls back to default provider if not configured

### 🧠 **Language Server Protocol (LSP) Features**
- **Automatic Setup**: Download and configure language servers automatically
- **Code Intelligence**: Real-time syntax highlighting, error detection, and diagnostics
- **Multi-Language**: Support for Rust, TypeScript, Python, Go, Java, C++, and more
- **Shell Integration**: Access LSP features directly from the terminal interface
- **Smart Completions**: Context-aware code completion suggestions
- **Error Highlighting**: Real-time error detection and inline diagnostics

### 🤖 **Subagent System Features**
- **Specialized Agents**: Deploy focused AI agents for specific development tasks
- **Tool Access Control**: Each agent type has carefully curated tool permissions for safety
- **Autonomous Operation**: Subagents work independently with their own reasoning and execution loops
- **Real-time Monitoring**: Track progress with live status updates and event streaming
- **Smart Task Assignment**: Automatically choose the best agent type based on task complexity
- **Five Agent Types**:
  - 🔍 **Code Analyzer**: Read-only analysis of code structure, patterns, and issues
  - 🧪 **Tester**: Create comprehensive test suites and run validation
  - 🔧 **Refactorer**: Make targeted code improvements and structural changes
  - 📝 **Documenter**: Generate and maintain project documentation
  - 🤖 **Custom Agent**: User-defined roles with flexible tool access

### 🖥️ **Interactive Shell Mode (Modern TUI)**
- **Command Block Interface**: Warp-like shell with visual command blocks and streaming output
- **AI Integration**: Use `@connect` and `@ <query>` for context-aware AI assistance
- **Real-time Tool Display**: Watch AI tool calls execute live with progress indicators
- **Diff Rendering**: File edits show compact diffs with +/- indicators for changes
- **Smart Autocomplete**: Tab completion for commands and file paths with popup UI
- **Scrolling Support**: Mouse scroll wheel and keyboard navigation
- **Context-Aware AI**: Automatically includes shell context (commands + outputs)
- **Git Safety**: Auto-commit disabled in shell mode to prevent unwanted changes

### 💻 **Standalone Coding CLI**
- **Direct AI Coding**: Full-featured coding assistant without external dependencies
- **Comprehensive Tool Suite**: Read, write, edit, glob, grep, list, todo, and web fetch
- **Multiple LLM Providers**: Claude, OpenAI, or Ollama (local models)
- **Privacy-First Option**: Run 100% locally with Ollama - no API costs, complete privacy
- **Beautiful TUI**: Modern terminal interface with professional styling and animations
- **File Picker**: Visual file selection with keyboard navigation and glob patterns

### ⚡ **Qwen Code CLI-Inspired Features**
- **Slash Commands**: Meta-level control with `/help`, `/stats`, `/chat save/resume/list`
- **At-Commands**: File context attachment with `@main.rs` or `@src/**/*.rs` (supports globs)
- **Shell Passthrough**: Direct command execution with `!cargo test`, `!git status`
- **Session Management**: Save conversations, resume later, track history
- **Memory System**: Project context via `.safe-coder/SAFE_CODER.md` file
- **Approval Modes**: Control AI execution (plan/default/auto-edit/yolo)
- **Custom Commands**: Create user-defined shortcuts for frequent operations
- **Statistics Tracking**: Monitor token usage, tool calls, session metrics

### 🎯 **Orchestrator Mode**
- **Multi-Agent Delegation**: Orchestrate Claude Code, Gemini CLI, GitHub Copilot, and Safe-Coder itself
- **Task Planning**: Automatically break down complex requests into manageable tasks
- **Workspace Isolation**: Each task runs in its own git worktree/branch
- **Parallel Execution**: Run up to 3 AI agents concurrently with intelligent throttling
- **Throttle Control**: Per-worker-type concurrency limits and start delays to respect rate limits
- **Worker Strategies**: Single-worker, round-robin, task-based, or load-balanced distribution
- **Automatic Merging**: Merge completed work back to main branch

### 🔒 **Git-Based Safety (Simplified Architecture)**
- **Direct Filesystem Access**: 20x faster than VM isolation while maintaining safety
- **Automatic Git Tracking**: Every change gets auto-committed with descriptive messages
- **Easy Rollback**: Use `/restore` or git commands to undo any changes
- **Change Transparency**: Review all modifications with standard Git tools
- **Approval Controls**: Multiple modes to control what AI can execute automatically

### 🎨 **Beautiful Interface**
- **Modern TUI Design**: Professional dark theme inspired by Google CLI and Claude Code
- **Dynamic ASCII Banner**: Large gradient banner with project context
- **Animated Processing**: Smooth braille spinners and real-time status updates
- **Cyberpunk Theme Option**: Neon colors with pulsing borders and glitch effects
- **Multi-Panel Layout**: Conversation, status, and tool execution panels
- **Real-time Updates**: Live monitoring of all operations and system status

## Architecture

Safe Coder now uses a simplified, high-performance architecture focused on **Git-based safety** instead of complex VM isolation:

```
┌─────────────────────────────────────────┐
│          Safe Coder CLI                 │
│  ┌───────────┐      ┌──────────────┐   │
│  │    LLM    │◄────►│ Tool Engine  │   │
│  │  Client   │      │ Read/Write/  │   │
│  │ (Claude/  │      │ Edit/Bash    │   │
│  │  OpenAI/  │      └──────┬───────┘   │
│  │  Ollama)  │             │           │
│  └───────────┘             │           │
│                            ▼           │
│  ┌─────────────────────────────────┐   │
│  │     Git Safety Manager          │   │
│  │  - Auto-commit after changes    │   │
│  │  - Snapshot before operations   │   │
│  │  - Easy rollback with /restore  │   │
│  │  - Change tracking & transparency  │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
                  │
                  ▼
        ┌──────────────────┐
        │  Project Files   │
        │  (Direct Access) │
        │  + Git Tracking  │
        └──────────────────┘
```

### Benefits of Simplified Architecture
- ⚡ **20x faster startup** (no VM overhead)
- ⚡ **10x faster tool execution** (direct filesystem access)
- 📉 **1,200+ lines removed** (simpler codebase)
- ✅ **Cross-platform compatibility** (works everywhere Git does)
- 🔧 **Better IDE integration** (file watchers, language servers work)
- 🎯 **Industry-standard safety** (Git-based rollback)

### Subagent System Architecture

Safe Coder includes a specialized subagent system for focused AI assistance:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Subagent Orchestrator                        │
│                                                                  │
│  ┌──────────────┐    ┌──────────────────────────────────────┐   │
│  │   Planner    │───►│         Task Analyzer                │   │
│  │  (Analyze    │    │  ┌──────────────────────────────────┐│   │
│  │   request)   │    │  │   Complexity Scoring Engine     ││   │
│  └──────────────┘    │  │   - Code analysis: simple       ││   │
│                       │  │   - Testing: medium            ││   │
│                       │  │   - Refactoring: complex       ││   │
│                       │  └──────────────────────────────────┘│   │
│                       └──────────────────────────────────────┘   │
│                                    │                             │
│         ┌──────────────────────────┼─────────────────────┐      │
│         ▼                          ▼                     ▼      │
│  ┌─────────────┐           ┌─────────────┐        ┌──────────┐ │
│  │🔍 Analyzer  │           │🧪 Tester    │        │🔧 Refactor│ │
│  │(Read-only)  │           │(Create tests│        │(Edit code)│ │
│  │- read_file  │           │- read/write │        │- read/edit│ │
│  │- glob/grep  │           │- bash       │        │- bash     │ │
│  │- bash       │           │)            │        │)          │ │
│  └─────────────┘           └─────────────┘        └──────────┘ │
│                                                                  │
│  ┌─────────────┐           ┌─────────────┐                     │
│  │📝 Documenter│           │🤖 Custom    │                     │
│  │(Write docs) │           │(User-defined│                     │
│  │- read/write │           │- read_file  │                     │
│  │- edit_file  │           │- glob/grep  │                     │
│  │- bash       │           │- bash)      │                     │
│  └─────────────┘           └─────────────┘                     │
└─────────────────────────────────────────────────────────────────┘
```

**Subagent Safety Model:**
- 🔒 **Tool Restrictions**: Each subagent type has limited tool access
- 🔍 **Read-Only Analysis**: Code Analyzer cannot modify files
- ⏱️ **Timeout Control**: Maximum execution time per subagent (5 min default)
- 🔄 **Iteration Limits**: Maximum reasoning loops to prevent runaway processes
- 📊 **Progress Monitoring**: Real-time status updates and event streaming

### Multi-Agent Orchestration

For complex tasks, Safe Coder can still orchestrate multiple AI agents:

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

1. **Git**: Required for change tracking and safety (usually pre-installed)

2. **API Key or Local Setup**: Choose one option:
   - **OpenRouter API key** for 75+ models (recommended - see [OpenRouter Setup](#openrouter-75-models))
   - **Anthropic API key** for Claude models directly
   - **OpenAI API key** for GPT models  
   - **Ollama** for local, private AI (see [Local AI Setup](#local-ai-with-ollama))

3. **For Orchestrator Mode** (optional):
   - [Claude Code](https://docs.anthropic.com/en/docs/claude-code): `npm install -g @anthropic-ai/claude-code`
   - [Gemini CLI](https://github.com/google/gemini-cli): Install from official repository
   - [GitHub Copilot](https://cli.github.com/): `gh extension install github/gh-copilot`

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

### Shell Mode (Interactive Terminal)

Start the modern shell with command blocks and AI integration:

```bash
# Start the shell TUI (default)
safe-coder shell

# Start shell in a specific directory
safe-coder shell --path /path/to/project

# Use legacy text-based shell (no TUI)
safe-coder shell --no-tui
```

**Shell TUI Features:**
- **Command Blocks**: Each command gets its own visual block with bordered output
- **Real-time Streaming**: See command output as it happens
- **AI Tool Execution**: Watch AI tools execute live with diff previews
- **Smart Autocomplete**: Tab completion for commands and file paths
- **Context-Aware AI**: Shell history automatically included in AI queries

**AI Commands in Shell:**

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

Start an AI coding session with full tool capabilities:

```bash
# Start TUI chat session
safe-coder chat

# Start in specific directory
safe-coder chat --path /path/to/project

# Classic CLI mode (no TUI)
safe-coder chat --tui false

# Use plan mode (preview tools before execution)
safe-coder chat --mode plan

# Demo mode (showcases all TUI features)
safe-coder chat --demo
```

**Advanced Chat Features:**

| Command Type | Example | Description |
|--------------|---------|-------------|
| **Slash Commands** | `/help`, `/stats`, `/chat save` | Meta-level control |
| **Skill Commands** | `/skill list`, `/skill activate rust` | Manage knowledge skills |
| **At-Commands** | `@main.rs`, `@src/**/*.rs` | Attach file context |
| **Shell Commands** | `!cargo test`, `!git status` | Execute shell commands |
| **Custom Commands** | `/test`, `/refactor <fn>` | User-defined shortcuts |

**Approval Modes:**
- `plan` - Show execution plan, ask for approval
- `default` - Ask before each tool (recommended)
- `auto-edit` - Auto-approve file operations only  
- `yolo` - Auto-approve everything (use with caution)

### Desktop App & HTTP Server

Safe Coder includes an HTTP/WebSocket server to integrate with a desktop application (Tauri-based) or other clients. The server exposes REST endpoints and real-time event streams (SSE) for sessions, messages, file changes, and a PTY WebSocket for terminal access.

Start the server with:

```bash
# Start the HTTP server (default: 127.0.0.1:9876)
safe-coder serve

# Custom host/port and enable CORS for development
safe-coder serve --host 0.0.0.0 --port 9876 --cors
```

Endpoints include (examples):
- GET /api/health - health check
- GET /api/config - current config
- GET/POST /api/sessions - list/create sessions
- GET/POST /api/sessions/:id/messages - send/receive messages
- GET /api/sessions/:id/events - Server-Sent Events (SSE) stream for real-time updates
- GET /api/sessions/:id/pty - PTY WebSocket for terminal access

This server is used by the desktop frontend in the `desktop/` folder. By default the server binds to 127.0.0.1 and CORS is disabled; enable --cors for local development if the frontend is served from a different origin.


### Orchestrate Mode (Multi-Agent)

Delegate complex tasks to multiple AI agents working in parallel:

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

### Subagent Mode (Specialized AI Assistants)

Deploy specialized AI subagents for focused tasks:

```bash
# Analyze code structure and patterns (read-only)
safe-coder subagent analyze "Review the authentication system for security issues"

# Create comprehensive tests
safe-coder subagent test "Add unit tests for the user service module"

# Refactor existing code
safe-coder subagent refactor "Extract the database logic into a separate module"

# Generate documentation
safe-coder subagent document "Create API documentation for all endpoints"

# Custom subagent with specific role
safe-coder subagent custom --role "security auditor" "Check for SQL injection vulnerabilities"

# Specify file patterns to focus on
safe-coder subagent analyze --files "src/**/*.rs" "Find performance bottlenecks in Rust code"
```

**Subagent Commands in Chat Mode:**

| Command | Description | Example |
|---------|-------------|---------|
| `/subagent <type> <task>` | Deploy a specialized subagent | `/subagent test "Add tests for auth module"` |
| `/analyze <task>` | Quick code analysis (read-only) | `/analyze "Find potential bugs"` |
| `/test <task>` | Create and run tests | `/test "Cover edge cases for user login"` |
| `/refactor <task>` | Refactor existing code | `/refactor "Simplify error handling"` |
| `/document <task>` | Generate documentation | `/document "Create README for this module"` |

**Subagent Safety Features:**
- **Tool Restrictions**: Each subagent type has limited tool access for safety
- **Read-Only Analysis**: Code Analyzer can only read files, never modify
- **Isolated Execution**: Each subagent runs in its own context
- **Progress Monitoring**: Real-time updates on subagent status and actions

### Local AI with Ollama

Run Safe Coder completely locally for privacy and cost savings:

```bash
# 1. Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# 2. Pull a coding model
ollama pull deepseek-coder:6.7b-instruct

# 3. Configure Safe Coder for Ollama
safe-coder config set llm.provider ollama
safe-coder config set llm.model "deepseek-coder:6.7b-instruct"

# 4. Start coding with local AI
safe-coder chat --path /your/project
```

**Recommended Models for Coding:**
- `deepseek-coder:6.7b-instruct` - Excellent balance (4GB, ~8GB RAM)
- `qwen2.5-coder:7b-instruct` - Very fast inference (4.7GB, ~8GB RAM) 
- `codellama:13b-instruct` - Higher quality (7.3GB, ~16GB RAM)
- `qwen2.5-coder:32b-instruct` - Best quality (19GB, ~32GB RAM)

See the [Ollama Setup Guide](OLLAMA_SETUP.md) for detailed instructions.

### OpenRouter (75+ Models)

Access 75+ AI models through a single API key:

```bash
# 1. Get your API key from https://openrouter.ai/keys

# 2. Set the environment variable
export OPENROUTER_API_KEY="sk-or-v1-..."

# 3. Start coding (auto-detects OpenRouter)
safe-coder chat --path /your/project

# Or explicitly configure
safe-coder config set llm.provider openrouter
safe-coder config set llm.model "anthropic/claude-3.5-sonnet"
```

**Popular Models Available:**

| Model | ID | Best For |
|-------|-----|----------|
| Claude 3.5 Sonnet | `anthropic/claude-3.5-sonnet` | General coding (default) |
| Claude 3 Opus | `anthropic/claude-3-opus` | Complex reasoning |
| GPT-4o | `openai/gpt-4o` | Fast, capable |
| GPT-4 Turbo | `openai/gpt-4-turbo` | Long context |
| Gemini Pro 1.5 | `google/gemini-pro-1.5` | Multi-modal |
| Llama 3.1 405B | `meta-llama/llama-3.1-405b-instruct` | Open source, powerful |
| Llama 3.1 70B | `meta-llama/llama-3.1-70b-instruct` | Open source, fast |
| DeepSeek Coder | `deepseek/deepseek-coder` | Code-specialized |
| Mixtral 8x22B | `mistralai/mixtral-8x22b-instruct` | Fast, efficient |
| Qwen 2 72B | `qwen/qwen-2-72b-instruct` | Multilingual |

**Benefits:**
- 🎯 **One API key** for all models - no need for separate accounts
- 💰 **Pay-per-use** - only pay for what you use
- 🔄 **Automatic fallback** - if a model is down, routes to a similar one
- 📊 **Usage dashboard** - track costs and usage at openrouter.ai

See the full model list at [openrouter.ai/models](https://openrouter.ai/models).

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

┌─ Command Block 4 ───────────────────────────────────────────┐
│ 🤖 my-project (main) $ @orchestrate add user auth and tests │
│ ┌─ Orchestration ────────────────────────────────────────┐  │
│ │ 🎯 Planning task: add user auth and tests              │  │
│ │                                                         │  │
│ │ 📋 Breaking down into 2 tasks:                         │  │
│ │   1. Add user authentication system                    │  │
│ │   2. Write comprehensive auth tests                    │  │
│ │                                                         │  │
│ │ 🚀 Starting workers...                                 │  │
│ │ ├─ Task 1: ClaudeCode → .safe-coder-workspaces/task-1  │  │
│ │ └─ Task 2: ClaudeCode → .safe-coder-workspaces/task-2  │  │
│ │                                                         │  │
│ │ ✓ Task 1 completed                                     │  │
│ │ ✓ Task 2 completed                                     │  │
│ │                                                         │  │
│ │ 📊 Results: 2/2 successful                             │  │
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

### Subagent Mode

```
🤖 Safe Coder Subagent System
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Project: /home/user/my-project
Available agents: Analyzer, Tester, Refactorer, Documenter, Custom

chat> /subagent analyze "Review the authentication system for security issues"

🔍 Deploying Code Analyzer subagent...
┌─ Subagent: analyzer-a1b2c3 ─────────────────────────────────┐
│ 🧠 Analyzing codebase for security issues in auth system... │
│                                                              │
│ ✓ Reading src/auth/mod.rs                                   │
│ ✓ Reading src/auth/tokens.rs                               │
│ ✓ Analyzing password hashing implementation                │
│ ✓ Checking session management                              │
│                                                              │
│ 📊 Analysis Results:                                        │
│ ┌─ Security Issues Found ─────────────────────────────────┐ │
│ │ ⚠️  Hardcoded JWT secret in tokens.rs:42               │ │  
│ │ ⚠️  Missing rate limiting on login endpoint            │ │
│ │ ⚠️  Session tokens never expire                        │ │
│ │ ✓  Password hashing uses bcrypt (good)                 │ │
│ │ ✓  SQL injection protection in place                   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                              │
│ 📝 Recommendations:                                         │
│ • Move JWT secret to environment variable                   │
│ • Implement login rate limiting                             │
│ • Add session expiration (24hr recommended)                 │
└──────────────────────────────────────────────────────────────┘

✓ Analyzer completed in 45 seconds (5 files read, 0 modified)

chat> /subagent test "Add unit tests for the auth module"

🧪 Deploying Tester subagent...
┌─ Subagent: tester-d4e5f6 ──────────────────────────────────┐
│ 🧠 Creating comprehensive tests for auth module...         │
│                                                              │
│ ✓ Reading existing auth code                               │
│ ✓ Analyzing test coverage gaps                             │
│ ✓ Writing tests/auth_test.rs                               │
│ ✓ Adding password validation tests                         │
│ ✓ Adding token generation tests                            │
│ ✓ Adding login flow tests                                  │
│                                                              │
│ 🏃 Running tests...                                         │
│ ┌─ Test Results ──────────────────────────────────────────┐ │
│ │ test auth::test_password_hashing ... ok                 │ │
│ │ test auth::test_invalid_password ... ok                 │ │
│ │ test auth::test_token_generation ... ok                 │ │
│ │ test auth::test_login_success ... ok                    │ │
│ │ test auth::test_login_failure ... ok                    │ │
│ │                                                          │ │
│ │ test result: ok. 5 passed; 0 failed; 0 ignored         │ │
│ └──────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘

✓ Tester completed in 2 minutes (8 files read, 1 modified)
  Created: tests/auth_test.rs (156 lines, 5 test functions)

chat> exit
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
# Provider options: "anthropic", "openai", "openrouter", "ollama", "github-copilot"
provider = "openrouter"
api_key = "sk-or-v1-..."  # Or set OPENROUTER_API_KEY env var
model = "anthropic/claude-3.5-sonnet"  # Any OpenRouter model ID
max_tokens = 8192

# Alternative: Direct Anthropic
# provider = "anthropic"
# api_key = "sk-ant-..."
# model = "claude-sonnet-4-20250514"

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
safe_coder_cli_path = "safe-coder"  # Path to Safe-Coder CLI (for self-orchestration)
gh_cli_path = "gh"              # Path to GitHub CLI (for Copilot)
max_workers = 3                 # Maximum concurrent workers (up to 3)
default_worker = "claude"       # Default: "claude", "gemini", "safe-coder", or "github-copilot"
worker_strategy = "single"      # Strategy: "single", "round-robin", "task-based", or "load-balanced"
enabled_workers = ["claude"]    # Workers to use for multi-worker strategies
use_worktrees = true            # Use git worktrees for isolation

# Throttle limits for controlling worker concurrency by type
[orchestrator.throttle_limits]
claude_max_concurrent = 2       # Max concurrent Claude workers
gemini_max_concurrent = 2       # Max concurrent Gemini workers
safe_coder_max_concurrent = 2   # Max concurrent Safe-Coder workers
copilot_max_concurrent = 2      # Max concurrent GitHub Copilot workers
start_delay_ms = 100            # Delay between starting workers (ms)

# Per-subagent LLM configuration (optional - falls back to main [llm] if not set)
[subagents.analyzer]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096

[subagents.tester]
provider = "openai"
model = "gpt-4o"
max_tokens = 4096

[subagents.refactorer]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[subagents.documenter]
provider = "ollama"
model = "llama3.1:8b"
max_tokens = 4096

[subagents.custom]
provider = "openrouter"
model = "anthropic/claude-3.5-sonnet"
max_tokens = 4096
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

### 🤖 **Subagent System Flow**

1. **Task Analysis**: Request is analyzed for complexity and categorized by task type
2. **Agent Selection**: The most appropriate subagent type is chosen automatically:
   - Simple analysis → Code Analyzer (read-only)
   - Test creation → Tester (read/write/bash)
   - Code changes → Refactorer (read/edit/bash)
   - Documentation → Documenter (read/write/edit/bash)
3. **Tool Restriction**: Subagent is limited to only its allowed tools for safety
4. **Autonomous Execution**: Subagent works independently with its own reasoning loop
5. **Progress Monitoring**: Real-time events track subagent thinking, tool use, and progress
6. **Result Collection**: Summary and detailed results are collected upon completion

**Subagent Safety Controls:**
- ⏱️ **Timeout Protection**: 5-minute default timeout prevents runaway processes
- 🔄 **Iteration Limits**: Maximum 15 reasoning loops to prevent infinite cycles
- 🔒 **Tool Sandboxing**: Each subagent type has restricted tool access
- 📊 **Activity Monitoring**: All tool calls and outputs are logged and displayed

### ⚡ **Parallel Execution with Throttling**

Safe Coder can run up to 3 CLI agents in parallel, with intelligent throttling:

- **Global Concurrency Limit**: Maximum of 3 workers running simultaneously
- **Per-Worker-Type Limits**: Control how many Claude, Gemini, Copilot, or Safe-Coder workers can run at once
- **Start Delay**: Configurable delay between starting workers

### 🔀 **Worker Distribution Strategies**

Choose how tasks are distributed across multiple worker types:

| Strategy | Description |
|----------|-------------|
| `single` | Use only the default worker for all tasks (default) |
| `round-robin` | Distribute tasks evenly across all enabled workers |
| `task-based` | Assign workers based on task complexity (simple → Copilot, complex → Claude) |
| `load-balanced` | Assign to workers with the fewest queued tasks |

Configure via `worker_strategy` and `enabled_workers` in your config file.

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
│   │   ├── mod.rs           # Orchestrator coordinator + worker strategies
│   │   ├── planner.rs       # Task decomposition
│   │   ├── worker.rs        # CLI workers (Claude, Gemini, Copilot, Safe-Coder)
│   │   ├── workspace.rs     # Git worktree manager
│   │   └── task.rs          # Task definitions
│   ├── session/             # Chat session management
│   ├── llm/                 # LLM client integrations
│   ├── tools/               # Agent tools
│   │   ├── mod.rs           # Tool registry and dispatch
│   │   ├── bash.rs          # Shell command execution
│   │   ├── read.rs          # File reading
│   │   ├── write.rs         # File writing
│   │   ├── edit.rs          # File editing with diffs
│   │   ├── glob.rs          # File pattern matching
│   │   ├── grep.rs          # Content search
│   │   ├── ast_grep.rs      # Structural code search (tree-sitter)
│   │   ├── list.rs          # Directory listing
│   │   ├── todo.rs          # Task tracking
│   │   └── webfetch.rs      # Web content retrieval
│   ├── tui/                 # Terminal UI
│   │   ├── shell_ui.rs      # Main shell interface
│   │   ├── shell_runner.rs  # Shell command runner
│   │   ├── file_picker.rs   # Visual file selection
│   │   └── autocomplete.rs  # Tab completion
│   ├── context/             # Project context awareness
│   ├── loop_detector/       # AI loop detection
│   ├── permissions/         # Permission mode handling
│   ├── prompts/             # System prompts
│   ├── git/                 # Git change tracking
│   ├── skills/              # Skill system for knowledge injection
│   │   └── mod.rs           # Skill loading and management
│   └── hooks/               # Lifecycle hooks system
│       ├── mod.rs           # Hook types and exports
│       ├── types.rs         # Hook trait and context definitions
│       ├── builtin.rs       # Built-in hooks (comment checker, etc.)
│       └── manager.rs       # Hook registration and execution
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
- [x] File picker with visual selection UI
- [x] Expanded tool suite (glob, grep, list, todo, webfetch)
- [x] Multiple permission modes (plan/default/auto-edit/yolo)
- [x] Inline bash tool streaming
- [x] Smarter AI with loop detection and context awareness
- [x] Better word wrapping in TUI
- [x] Inline LLM reasoning display between tool calls
- [x] Shell-integrated orchestration (`@orchestrate` command)
- [x] GitHub Copilot worker support
- [x] Worker distribution strategies (round-robin, task-based, load-balanced)
- [x] Self-orchestration (Safe-Coder as a worker)
- [x] Language Server Protocol (LSP) support with automatic downloads
- [x] OpenCode-inspired UI theme
- [x] **Specialized subagent system** - Deploy focused AI agents for specific tasks
- [x] **Enhanced task planning** - Complexity scoring and intelligent agent assignment
- [x] **Tool-restricted agents** - Safety through limited tool access per agent type
- [x] **Multi-model subagents** - Configure different LLM providers per subagent type
- [x] **AST-Grep tool** - Structural code search using tree-sitter
- [x] **Hooks system** - Lifecycle hooks for extensibility (pre/post tool, file write, etc.)
- [x] **Skill system** - Loadable knowledge files with auto-activation
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

- Orchestrates [Claude Code](https://docs.anthropic.com/en/docs/claude-code), [Gemini CLI](https://github.com/google/gemini-cli), and [GitHub Copilot](https://cli.github.com/)
- TUI powered by [Ratatui](https://github.com/ratatui-org/ratatui)
- Diff rendering powered by the [Similar](https://github.com/mitsuhiko/similar) crate
- AST parsing powered by [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
- Built with Rust for performance and safety
