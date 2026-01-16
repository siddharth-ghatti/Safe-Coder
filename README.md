# Safe Coder

Safe Coder is a Rust-powered, AI-first shell and multi-agent orchestrator for safe and efficient code automation. It combines a terminal user interface (TUI), project-aware tooling, planning/agent modes, and an HTTP server for desktop integration.

Version: 0.2.0

## Key Features

- Interactive TUI shell with planning (PLAN) and execution (BUILD) agent modes
- Multi-agent orchestration and isolated worktrees for safe parallel edits
- **Subagent system** for spawning specialized agents (code analysis, testing, refactoring, documentation)
- LSP integration and code intelligence (Rust, JS/TS, Python, Go, ...)
- Built-in HTTP/WebSocket server for desktop (Tauri) and third-party clients
- Extensible skills, hooks, and permission controls for safe automation

## Quick Start

### Prerequisites

- Rust toolchain (install via https://rustup.rs/)
- Optional: API keys for hosted LLMs (see Environment below)

### Build from Source

```bash
# Build release binary
cargo build --release
# The binary will be in target/release/safe-coder
```

### Run (release binary)

```bash
# Start interactive shell in your project directory
cd your-project

# Run the release binary (basic mode)
./target/release/safe-coder

# Start with AI connected (if configured)
./target/release/safe-coder --ai

# Start built-in HTTP server for desktop integration (default: 127.0.0.1:9876)
./target/release/safe-coder serve
```

### Run (cargo run for development)

```bash
# Start via cargo (useful during development)
# Any flags after -- are forwarded to the application
cargo run --release -- --ai

# or run the server
cargo run --release -- serve
```

## Environment

Set one of the supported API keys if you want to use hosted LLMs. Example:

```bash
export OPENROUTER_API_KEY="sk-or-..."    # OpenRouter (many models)
# or
export ANTHROPIC_API_KEY="sk-ant-..."    # Claude
```

## Subagents and Orchestration

Safe Coder includes a **subagent system** that allows spawning specialized, autonomous agents for focused tasks. Subagents run within the same process and share context with the parent session.

### What Are Subagents?

Subagents are specialized agents that handle specific use cases autonomously. They execute in a bounded conversation loop with their own LLM context and tool permissions. The subagent code lives in `src/subagent/`:

- `src/subagent/mod.rs` - Module exports and types
- `src/subagent/types.rs` - `SubagentKind`, `SubagentScope`, `SubagentResult`, `SubagentEvent`
- `src/subagent/tool.rs` - The `subagent` tool implementation for spawning subagents
- `src/subagent/executor.rs` - `SubagentExecutor` that runs the conversation loop
- `src/subagent/prompts.rs` - System prompts for each subagent kind

### Available Subagent Kinds

| Kind | Description | Permissions |
|------|-------------|-------------|
| `code_analyzer` | Analyzes code structure, patterns, and potential issues | **Read-only**: `read_file`, `list`, `glob`, `grep`, `bash` |
| `tester` | Creates and runs tests | **Read-write**: `read_file`, `list`, `glob`, `grep`, `write_file`, `edit_file`, `bash` |
| `refactorer` | Makes targeted code improvements and refactoring | **Read-write**: `read_file`, `list`, `glob`, `grep`, `edit_file`, `bash` |
| `documenter` | Generates and updates documentation | **Read-write**: `read_file`, `list`, `glob`, `grep`, `write_file`, `edit_file`, `bash` |
| `explorer` | Navigates codebase, finds patterns, answers "where is X" questions | **Read-only**: `read_file`, `list`, `glob`, `grep`, `bash`, `ast_grep`, `code_search` |
| `custom` | User-defined role with custom behavior | **Basic**: `read_file`, `list`, `glob`, `grep`, `bash` |

### Spawning a Subagent

The AI can spawn a subagent using the `subagent` tool during a session. Tool parameters:

| Parameter | Required | Description |
|-----------|----------|-------------|
| `kind` | Yes | One of: `code_analyzer`, `tester`, `refactorer`, `documenter`, `explorer`, `custom` |
| `task` | Yes | The specific task for the subagent to accomplish |
| `role` | No | For `custom` kind only: describes the role and capabilities |
| `file_patterns` | No | File patterns to focus on (e.g., `["src/**/*.rs", "tests/**/*.rs"]`) |

**Example JSON (tool invocation):**

```json
{
  "kind": "tester",
  "task": "Write unit tests for the user authentication module",
  "file_patterns": ["src/auth/**/*.rs"]
}
```

**Example CLI context:**

When using Safe Coder interactively, you might prompt the AI:

```
> Please analyze the error handling in src/llm/ and suggest improvements

The AI may invoke:
subagent(kind="code_analyzer", task="Analyze error handling patterns in src/llm/ and identify potential improvements", file_patterns=["src/llm/**/*.rs"])
```

### Plan Executor Integration

The `SubagentPlanExecutor` in `src/unified_planning/executors/subagent.rs` provides integration with the unified planning system. This executor can delegate plan steps to specialized subagents and supports parallel execution.

> **Note:** Full integration between the plan executor and `src/subagent/executor.rs` is in progress. The placeholder currently simulates subagent execution.

### Per-Subagent Model Configuration

Each subagent kind can use a different LLM model. Configure this in your `config.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-3.5-sonnet"
max_tokens = 8192

[subagents.analyzer]
provider = "openai"
model = "gpt-4o-mini"
max_tokens = 2048

[subagents.tester]
provider = "anthropic"
model = "claude-3-haiku"
max_tokens = 4096
```

The system uses `config.get_subagent_model(kind)` to retrieve per-subagent model configuration, falling back to the main LLM config if not specified.

### Safety and Sandboxing

> **Warning:** Subagents run in-process and have access to powerful tools including `bash` and file editing (`edit_file`, `write_file`).

Recommendations for production use:

- **Configure tool permissions:** Review and restrict tools available to each subagent kind in `src/subagent/types.rs`
- **Path normalization:** Ensure file paths are validated and normalized to prevent directory traversal
- **Sandbox environments:** Run Safe Coder in a sandboxed environment (Docker, VM, or restricted user) when processing untrusted input
- **Timeout limits:** Subagents have a default 5-minute timeout and 15-iteration limit; adjust via `SubagentScope` if needed
- **Review bash commands:** The `bash` tool is available to most subagent kinds; consider restricting or auditing its use

## Development

- The project uses the Rust 2021 edition and tokio for async runtime.
- Run unit/integration tests with:

```bash
cargo test
```

- Run the linter/formatter during development:

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
```

### Subagent Tests

Subagent configuration tests are located in `tests/integration/subagent_config_tests.rs`. These tests cover:

- Per-subagent model configuration
- Serialization/deserialization of subagent configs
- Provider fallback behavior

**Recommended additions:**
- Tests for `SubagentExecutor` conversation loop and tool filtering
- Tests for cancellation and timeout handling
- Integration tests with mock LLM clients

## Project Layout (high level)

| Directory | Description |
|-----------|-------------|
| `src/main.rs` | Binary entrypoint |
| `src/lib.rs` | Library layer |
| `src/shell/` | TUI and shell logic |
| `src/orchestrator/` | Planning and agent orchestration |
| `src/tui/` | UI components |
| `src/subagent/` | Subagent module - specialized agent roles ([mod.rs](src/subagent/mod.rs), [types.rs](src/subagent/types.rs), [tool.rs](src/subagent/tool.rs), [executor.rs](src/subagent/executor.rs)) |
| `src/unified_planning/` | Unified planning system with executors (includes [executors/subagent.rs](src/unified_planning/executors/subagent.rs)) |
| `src/llm/` | Language model clients |
| `src/tools/` | Tool implementations (bash, file ops, grep, etc.) |
| `tests/integration/` | Integration tests (includes [subagent_config_tests.rs](tests/integration/subagent_config_tests.rs)) |

## Contributing

Contributions are welcome. Please open issues or PRs. Keep changes small and focused. Follow the repository coding conventions and include tests where appropriate.

## License

MIT — see the LICENSE file for details.

## Credits

Built with Ratatui, Tree-sitter, and other open-source Rust crates.

## Contact

Project repository maintained in this workspace. For questions, open an issue.
