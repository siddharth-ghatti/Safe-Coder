# Safe Coder

Safe Coder is a Rust-powered, AI-first shell and multi-agent orchestrator for safe and efficient code automation. It combines a terminal user interface (TUI), project-aware tooling, planning/agent modes, and an HTTP server for desktop integration.

Version: 0.1.0

Key features

- Interactive TUI shell with planning (PLAN) and execution (BUILD) agent modes
- Multi-agent orchestration and isolated worktrees for safe parallel edits
- LSP integration and code intelligence (Rust, JS/TS, Python, Go, ...)
- Built-in HTTP/WebSocket server for desktop (Tauri) and third-party clients
- Extensible skills, hooks, and permission controls for safe automation

Quick start

Prerequisites

- Rust toolchain (install via https://rustup.rs/)
- Optional: API keys for hosted LLMs (see Environment below)

Build from source

```bash
# Build release binary
cargo build --release
# The binary will be in target/release/safe-coder
```

Run (release binary)

```bash
# Start interactive shell in your project directory
cd your-project
# Run the release binary
./target/release/safe-coder
# Start with AI connected (if configured)
./target/release/safe-coder --ai
# Start built-in HTTP server for desktop integration (default: 127.0.0.1:9876)
./target/release/safe-coder serve
```

Run (cargo run for development)

```bash
# Start via cargo (useful during development)
# Any flags after -- are forwarded to the application
cargo run --release -- --ai
# or run the server
cargo run --release -- -- serve
```

Environment

Set one of the supported API keys if you want to use hosted LLMs. Example:

```bash
export OPENROUTER_API_KEY="sk-or-..."    # OpenRouter (many models)
# or
export ANTHROPIC_API_KEY="sk-ant-..."    # Claude
```

Development

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

Project layout (high level)

- src/main.rs — binary entrypoint
- src/lib.rs — library layer
- src/shell — TUI and shell logic
- src/orchestrator — planning and agent orchestration
- src/tui — UI components
- src/subagents — specialized agent roles
- src/llm — language model clients

Contributing

Contributions are welcome. Please open issues or PRs. Keep changes small and focused. Follow the repository coding conventions and include tests where appropriate.

License

MIT — see the LICENSE file for details.

Credits

Built with Ratatui, Tree-sitter, and other open-source Rust crates.

Contact

Project repository maintained in this workspace. For questions, open an issue.
