# Safe Coder

**AI-powered coding assistant that defaults to YOLO mode.**

> **Why "Safe Coder"?** It's ironic. This started as an experiment in AI-assisted coding, and the name is tongue-in-cheek—the default mode is BUILD (aka YOLO mode), which auto-executes commands without asking. Use PLAN mode if you actually want safety. You've been warned.

Safe Coder is a terminal-first AI coding assistant that helps you write, analyze, and refactor code. It features an interactive TUI, a desktop app, and supports multiple LLM providers.

## Features

### Core Capabilities
- **Interactive TUI Shell** - Full terminal interface with syntax highlighting and streaming responses
- **Desktop App** - Cross-platform Tauri app for a native experience (macOS, Linux, Windows)
- **Multi-Provider Support** - Works with OpenRouter, Anthropic, OpenAI, Ollama, and GitHub Copilot
- **Plan & Build Modes** - Review changes before execution (Plan) or auto-execute (Build/YOLO - the default)

### AI-Powered Tools
- **Code Analysis** - Analyze code structure, patterns, and potential issues
- **File Operations** - Read, write, and edit files with AI assistance
- **Smart Search** - Glob patterns, grep, AST-based search, and multi-pattern code search
- **Bash Integration** - Execute shell commands with safety controls
- **Subagents** - Spawn specialized agents for testing, refactoring, documentation, and exploration

### Safety & Control
- **Dangerous Command Detection** - Warns before running risky commands (even in YOLO mode)
- **LSP Integration** - Real-time diagnostics and code intelligence (Rust, TypeScript, Python, Go, etc.)
- **Checkpoint System** - Automatically saves state for recovery
- **Permission Controls** - Fine-grained tool permissions per mode

## Quick Start

### Installation

**From Releases (Recommended):**
```bash
# Download the latest release for your platform from GitHub Releases
# Linux/macOS:
chmod +x safe-coder-*
sudo mv safe-coder-* /usr/local/bin/safe-coder

# Or use the Desktop App (.dmg, .AppImage, .msi)
```

**Build from Source:**
```bash
git clone https://github.com/yourusername/safe-coder
cd safe-coder
cargo build --release
# Binary: target/release/safe-coder
```

### Configuration

**Option 1: Environment Variables (Simplest)**
```bash
# Pick one provider:
export OPENROUTER_API_KEY="sk-or-..."    # OpenRouter (recommended - many models)
export ANTHROPIC_API_KEY="sk-ant-..."    # Anthropic Claude
export OPENAI_API_KEY="sk-..."           # OpenAI
```

**Option 2: Project Config File**

Create `safecoder.json` in your project directory:

```json
{
  "llm": {
    "provider": "openrouter",
    "model": "anthropic/claude-sonnet-4-20250514",
    "max_tokens": 8192,
    "base_url": null,
    "api_key": null
  },
  "git": {
    "auto_commit": false
  },
  "tools": {
    "bash_timeout_secs": 120,
    "max_output_bytes": 1048576,
    "warn_dangerous_commands": true
  },
  "lsp": {
    "enabled": true
  },
  "cache": {
    "enabled": true,
    "provider_native": true,
    "application_cache": true,
    "max_entries": 100,
    "ttl_minutes": 30
  },
  "build": {
    "enabled": true,
    "timeout_secs": 60
  }
}
```

**Config Priority:**
1. `safecoder.json` in project directory (highest)
2. `~/.config/safe-coder/config.toml` (global)
3. Environment variables for API keys
4. Default values

### Usage

```bash
# Start in current directory (defaults to BUILD/YOLO mode)
safe-coder

# Start with AI connected
safe-coder --ai

# Start HTTP server for desktop app
safe-coder serve
```

**In the TUI:**
- Type your request and press Enter
- Use `Ctrl+B` to toggle between Plan/Build modes
- Use `Ctrl+C` to cancel operations
- Use `Ctrl+Q` to quit

## Providers

| Provider | Env Variable | Example Model |
|----------|--------------|---------------|
| OpenRouter | `OPENROUTER_API_KEY` | `anthropic/claude-sonnet-4-20250514` |
| Anthropic | `ANTHROPIC_API_KEY` | `claude-sonnet-4-20250514` |
| OpenAI | `OPENAI_API_KEY` | `gpt-4o` |
| Ollama | (none needed) | `llama3.2` |
| GitHub Copilot | `GITHUB_COPILOT_TOKEN` | `gpt-4` |

## Desktop App

The Safe Coder desktop app provides a native experience with:
- Modern React-based UI
- Real-time streaming responses
- File diff viewer
- Project sidebar

Download from [GitHub Releases](https://github.com/yourusername/safe-coder/releases) or build locally:
```bash
cd desktop
npm install
npm run tauri:build
```

## Subagents

Safe Coder can spawn specialized subagents for focused tasks:

| Subagent | Use Case |
|----------|----------|
| `explorer` | Navigate codebase, find patterns, answer "where is X" |
| `code_analyzer` | Analyze code structure and identify issues |
| `tester` | Create and run tests |
| `refactorer` | Make targeted code improvements |
| `documenter` | Generate documentation |

The AI automatically uses subagents when appropriate, or you can explicitly request them.

## Coming Soon

- **Orchestrator Mode** - Delegate tasks to external CLI agents (Claude Code, Gemini CLI) for parallel execution
- **Enhanced Parallelization** - Better parallel task execution and workspace isolation
- **MCP Server Support** - Connect to Model Context Protocol servers for extended capabilities

## Development

```bash
# Run tests
cargo test

# Run with logging
RUST_LOG=safe_coder=debug cargo run

# Build desktop app in dev mode
cd desktop && npm run tauri:dev
```

## Project Structure

| Directory | Description |
|-----------|-------------|
| `src/` | Core Rust library and CLI |
| `src/tools/` | Tool implementations (bash, file ops, search) |
| `src/subagent/` | Subagent system |
| `src/llm/` | LLM provider clients |
| `src/tui/` | Terminal UI components |
| `desktop/` | Tauri desktop app |

## License

MIT - see LICENSE file.

## Contributing

Contributions welcome! Please open issues or PRs. Keep changes focused and include tests where appropriate.
