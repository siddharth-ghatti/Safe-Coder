# Changelog

All notable changes to Safe Coder will be documented in this file.

## [v2.3.0] - 2024-12-31

### 🧠 **Major Addition: Language Server Protocol (LSP) Support**

#### Added
- **Complete LSP integration** with automatic language server downloads
- **Multi-language support** for Rust, TypeScript, Python, Go, Java, C++, and more
- **Real-time code intelligence** - syntax highlighting, error detection, diagnostics
- **Smart code completion** with context-aware suggestions
- **Automatic LSP server management** - download, install, and configure language servers
- **Shell-integrated LSP features** accessible directly from the TUI

#### New Files
- `src/lsp/mod.rs` - Core LSP module (22 lines)
- `src/lsp/client.rs` - LSP client implementation (405 lines)
- `src/lsp/config.rs` - Configuration management (369 lines)
- `src/lsp/download.rs` - Automatic server downloads (710 lines)
- `src/lsp/manager.rs` - Server lifecycle management (580 lines)
- `src/lsp/protocol.rs` - Protocol message handling (313 lines)

#### Enhanced
- Shell UI integration with LSP status indicators
- Real-time error highlighting in terminal
- Configuration system extended for LSP settings

### 🎨 **UI/UX Improvements**

#### Changed
- **OpenCode-inspired theme** - Modern VS Code-style interface
- Redesigned shell UI (net -393 lines of code while adding features)
- Enhanced visual styling and user experience
- Improved code organization and maintainability

### 🔗 **Integration Enhancements**

#### Enhanced
- **Orchestration in shell** - `@orchestrate` command integration
- SVG icons for better visual feedback
- Seamless multi-agent delegation from shell interface

### Technical Details
- **Total lines added**: 2,618 lines
- **Files modified**: 12 files
- **New dependencies**: Added LSP-related crates
- **Performance**: Maintained startup speed with new features

---

## [v2.2.0] - 2024-12-29

### 🚀 **Orchestration Integration**
- Shell-integrated orchestration with `@orchestrate` command
- GitHub Copilot support as worker type
- Worker distribution strategies (single, round-robin, task-based, load-balanced)
- Self-orchestration capability
- Plan vs Act modes for task execution

### 🧠 **Smarter AI**
- Context-aware reasoning
- Loop detection prevention
- Inline bash streaming
- Improved word wrapping

### 🛠️ **Expanded Tool Suite**
- Glob search with `**/*.rs` syntax
- Grep search with regex support
- File listing with smart filtering
- Todo tracking for workflow management
- Web fetch capabilities

### 📁 **File Picker UI**
- Visual file selection interface
- Keyboard navigation support
- Glob pattern filtering

### 🔐 **Permission Modes**
- Plan mode (preview before execution)
- Default mode (ask before tools)
- Auto-edit mode (auto-approve file ops)
- YOLO mode (auto-approve all)

---

## [v2.1.0] - 2024-12-15

### ⚡ **Simplified Architecture**
- 20x faster startup (removed VM complexity)
- Git-based safety with auto-commits
- Cross-platform compatibility
- 1,200+ lines removed for cleaner codebase

### 🦙 **Local AI Support**
- Ollama integration for private AI
- Offline capability
- Multiple model support
- GPU acceleration

### 🎨 **Enhanced TUI**
- Dynamic ASCII banner with gradients
- Cyberpunk theme option
- Professional dark mode
- Animated processing indicators

### ⚡ **Qwen Code CLI Features**
- Slash commands for meta-control
- At-commands for context attachment
- Shell passthrough
- Session management

---

## [v2.0.0] - 2024-12-01

### 🖥️ **Interactive Shell Mode**
- Modern TUI with command blocks
- AI integration with `@connect`
- Real-time tool execution display
- Smart autocomplete with Tab completion
- Context-aware AI assistance

### 💻 **Standalone Coding CLI**
- Full-featured coding assistant
- Multiple LLM providers
- Beautiful TUI interface
- File picker with visual selection

### 🎯 **Orchestrator Mode**
- Multi-agent delegation
- Automatic task decomposition
- Workspace isolation
- Parallel execution

### 🔒 **Git-Based Safety**
- Direct filesystem access
- Automatic change tracking
- Easy rollback capabilities
- Change transparency

---

## [v1.0.0] - 2024-11-01

### Initial Release
- Basic AI coding assistant
- VM-based isolation
- Simple CLI interface
- Claude/OpenAI integration