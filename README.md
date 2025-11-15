# Safe Coder

An AI-powered coding assistant CLI built in Rust with **Firecracker microVM isolation**. Each coding session runs in an isolated Firecracker VM, providing strong security boundaries while the AI agent makes edits to your code.

## Features

### 🔒 **Security First**
- **Strict VM Isolation**: Agent operates ONLY in isolated sandbox, zero access to host filesystem
- **Git Change Tracking**: Every modification automatically tracked with git in the VM
- **Safe Sync Back**: Changes reviewed and synced to host only on exit
- **Rollback Support**: Undo any changes made within the VM session

### 🎨 **Beautiful Interface**
- **Cyberpunk TUI**: Modern neon-themed terminal UI with pulsing borders and animations
- **Multi-Panel Layout**: Conversation, VM status, and tool execution panels
- **Real-time Updates**: Live VM monitoring (status, uptime, memory, CPU)
- **Dynamic Processing**: Animated braille spinners and status messages

### 🤖 **AI-Powered Coding**
- **Multiple LLM Providers**: Claude, OpenAI, or Ollama (local models)
- **Privacy Option**: Run 100% locally with Ollama - no API costs, complete privacy
- **Full Tool Suite**: Read, write, edit files, and execute bash commands
- **Contextual Awareness**: Agent understands your codebase and makes intelligent changes
- **Auto-Commit**: Every tool execution is tracked in git with descriptive messages

## Architecture

```
┌─────────────────────────────────────────┐
│          Safe Coder CLI                 │
│  ┌───────────┐      ┌──────────────┐   │
│  │    LLM    │◄────►│  Tool Engine │   │
│  │  Client   │      │ Read/Write/  │   │
│  │ (Claude/  │      │ Edit/Bash    │   │
│  │  OpenAI)  │      └──────┬───────┘   │
│  └───────────┘             │           │
│                            │           │
│  ┌─────────────────────────▼─────────┐ │
│  │     Firecracker VM Manager        │ │
│  │  - Manages VM lifecycle           │ │
│  │  - File synchronization           │ │
│  │  - Command execution in VM        │ │
│  └───────────────────────────────────┘ │
└─────────────────────────────────────────┘
                  │
                  ▼
        ┌──────────────────┐
        │  Firecracker VM  │
        │   (per project)  │
        │  - Isolated env  │
        │  - Project files │
        └──────────────────┘
```

## Platform Support

| Platform | Firecracker | Docker |
|----------|-------------|--------|
| **Linux** | ✅ Native (best security) | ✅ Supported |
| **macOS** | ❌ Not supported | ✅ Supported (via Docker Desktop) |
| **Windows** | ❌ Not supported | ✅ Supported (via Docker Desktop) |

**Default behavior**: Safe Coder auto-selects the best backend for your platform:
- **Linux**: Firecracker (maximum security)
- **macOS/Windows**: Docker (only option)

You can override this in the config file.

## Prerequisites

### Option 1: Firecracker (Linux Only) - Maximum Security

Firecracker requires Linux and KVM. Install Firecracker:

```bash
# Download Firecracker binary
ARCH="$(uname -m)"
release_url="https://github.com/firecracker-microvm/firecracker/releases"
latest=$(basename $(curl -fsSLI -o /dev/null -w  %{url_effective} ${release_url}/latest))
curl -L ${release_url}/download/${latest}/firecracker-${latest}-${ARCH}.tgz \
| tar -xz

# Move to /usr/local/bin
sudo mv release-${latest}-${ARCH}/firecracker-${latest}-${ARCH} /usr/local/bin/firecracker
sudo chmod +x /usr/local/bin/firecracker
```

### 2. Kernel and Root Filesystem

You need a Linux kernel and root filesystem for the VMs:

```bash
# Create directory for VM assets
sudo mkdir -p /var/lib/safe-coder

# Download kernel (example using Ubuntu's kernel)
curl -fsSL -o /tmp/vmlinux https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/kernels/vmlinux.bin
sudo mv /tmp/vmlinux /var/lib/safe-coder/vmlinux

# Create or download a rootfs
# Option 1: Use a pre-built rootfs
curl -fsSL -o /tmp/rootfs.ext4 https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/rootfs/bionic.rootfs.ext4
sudo mv /tmp/rootfs.ext4 /var/lib/safe-coder/rootfs.ext4

# Option 2: Build your own (see Firecracker documentation)
```

### Option 2: Docker (All Platforms) - Cross-Platform

Docker works on Linux, macOS, and Windows:

```bash
# Linux
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER

# macOS/Windows
# Install Docker Desktop from https://www.docker.com/products/docker-desktop
```

Verify Docker is installed:
```bash
docker --version
```

### 3. LLM Provider Setup

Choose one of the following:

#### Option A: Cloud APIs (Anthropic/OpenAI)
- **Anthropic** (recommended): Get API key from https://console.anthropic.com/
- **OpenAI**: Get API key from https://platform.openai.com/

#### Option B: Local with Ollama (Free, Private)
- **Ollama**: Install from https://ollama.com
- No API key needed, runs 100% locally
- See [OLLAMA_SETUP.md](OLLAMA_SETUP.md) for detailed setup

## Installation

### From Source

```bash
# Clone the repository
git clone <your-repo-url>
cd safe-coder

# Build the project
cargo build --release

# Install the binary
sudo cp target/release/safe-coder /usr/local/bin/

# Or add to your PATH
export PATH="$PATH:$(pwd)/target/release"
```

## Configuration

### Initial Setup

```bash
# Configure your API key
safe-coder config --api-key YOUR_ANTHROPIC_API_KEY

# Optional: Change the model
safe-coder config --model claude-sonnet-4-20250514

# View current configuration
safe-coder config --show
```

### Configuration File

The configuration is stored in `~/.config/safe-coder/config.toml`:

```toml
[llm]
provider = "anthropic"  # Options: anthropic, openai, ollama
api_key = "your-api-key-here"  # Not needed for ollama
model = "claude-sonnet-4-20250514"
max_tokens = 8192
# base_url = "http://localhost:11434"  # For ollama or custom endpoints

# Isolation backend selection
[isolation]
# Options: "auto" (default), "firecracker", "docker"
# - auto: Firecracker on Linux, Docker on other platforms
# - firecracker: Force Firecracker (Linux only)
# - docker: Force Docker (all platforms)
backend = "auto"

# Firecracker configuration (Linux only)
[vm]
firecracker_bin = "/usr/local/bin/firecracker"
kernel_image = "/var/lib/safe-coder/vmlinux"
rootfs_image = "/var/lib/safe-coder/rootfs.ext4"
vcpu_count = 2
mem_size_mib = 512

# Docker configuration (all platforms)
[docker]
image = "ubuntu:22.04"
cpus = 2.0
memory_mb = 512
auto_pull = true
```

## Usage

### Start a Coding Session

```bash
# Use the beautiful TUI (default)
cd /path/to/your/project
safe-coder chat

# Or specify a path
safe-coder chat --path /path/to/project

# Use classic CLI mode
safe-coder chat --no-tui
```

### TUI Interface

The TUI provides a modern, multi-panel interface:

```
┌─────────────────────────────────────────────────────────────────┐
│                  🔥 Safe Coder | /path/to/project               │
├──────────────────────────────────────────┬──────────────────────┤
│ 💬 Conversation                          │ 🔥 VM Status         │
│ ────────────────────────────────────     │ 🟢 Status: Running   │
│ 👤 [12:30:45] You: Create hello.rs      │ ⏱️  Uptime: 5m 23s   │
│                                          │ 💾 Memory: 512 MB    │
│ 🤖 [12:30:47] Assistant: I'll create    │ ⚙️  vCPUs: 2         │
│    a hello.rs file with a simple        │                      │
│    "Hello, World!" program.             ├──────────────────────┤
│                                          │ 🔧 Recent Tools      │
│ ✓ Using write_file tool                 │ ✓ write_file         │
│                                          │ ✓ bash               │
│                                          │ ✓ read_file          │
│                                          │                      │
├──────────────────────────────────────────┴──────────────────────┤
│ ❯ your message here█                                            │
├─────────────────────────────────────────────────────────────────┤
│ ^C Exit │ ↑↓ Scroll │ Tab Switch Panel │ Status: Ready         │
└─────────────────────────────────────────────────────────────────┘
```

**Keyboard Shortcuts:**
- `Ctrl+C`: Exit the application
- `↑/↓`: Scroll through conversation
- `PageUp/PageDown`: Scroll by page
- `Tab`: Switch between panels
- `Enter`: Send message

### Example Session

```
🔥 Safe Coder - AI Coding Assistant with Firecracker VM Isolation
Project: /home/user/my-project
Type 'exit' or 'quit' to end the session

> Create a new file called hello.rs with a simple "Hello, World!" program

[AI creates the file using the write_file tool]

> Now compile and run it

[AI uses the bash tool to run: rustc hello.rs && ./hello]

> Add error handling to the program

[AI edits the file using the edit_file tool]

> exit

Stopping VM and cleaning up...
Goodbye!
```

### Available Tools

The AI assistant has access to these tools:

- **read_file**: Read file contents with line numbers
- **write_file**: Create or overwrite files
- **edit_file**: Make precise string replacements in files
- **bash**: Execute shell commands in the VM

## How It Works

### 🔒 **VM Isolation Flow**

1. **Initialization** (Safe Sandbox Creation):
   ```
   - Create temp VM sandbox: /tmp/safe-coder-{uuid}/
   - Copy entire project to sandbox
   - Initialize git repository in sandbox
   - Initial commit: "Initial snapshot - Safe Coder VM"
   - VM ready: agent confined to sandbox
   ```

2. **Agent Operations** (Isolated Execution):
   ```
   - ALL tools execute ONLY in VM sandbox
   - read_file: Reads from /tmp/safe-coder-{uuid}/
   - write_file: Writes to /tmp/safe-coder-{uuid}/
   - edit_file: Edits in /tmp/safe-coder-{uuid}/
   - bash: Executes in /tmp/safe-coder-{uuid}/
   - Auto-commit after each tool: "Agent executed: tool1, tool2"
   ```

3. **Cleanup** (Safe Sync Back):
   ```
   - Get git change summary from VM
   - Display changes to user
   - Sync files back to host (excluding .git)
   - Shutdown VM and cleanup sandbox
   - Host project updated with reviewed changes
   ```

### 📝 **Git Tracking**

Every change is version-controlled:

```bash
# In VM sandbox:
git log --oneline

d4e2b8c Agent executed: write, edit
a1b2c3d Agent executed: bash
f5e6d7c Initial snapshot - Safe Coder VM
```

**Read more:**
- [VM_ISOLATION.md](VM_ISOLATION.md) - Detailed security architecture
- [DOCKER_BACKEND.md](DOCKER_BACKEND.md) - Cross-platform Docker isolation

## Security

Safe Coder provides **defense-in-depth security**:

### 🔒 **VM Isolation**
- Each session runs in a fresh Firecracker microVM
- Agent has **ZERO** access to host filesystem
- All operations confined to `/tmp/safe-coder-{uuid}/` sandbox
- No fallback to host paths (strict isolation enforced)

### 📝 **Change Tracking**
- Git initialized automatically in VM on startup
- Auto-commit after every tool execution
- Full audit trail of all agent actions
- Rollback support for any mistakes

### 🛡️ **Resource Limits**
- Configurable CPU limits (default: 2 vCPUs)
- Configurable memory limits (default: 512 MB)
- No network access by default
- Temporary sandbox cleanup on exit

### ✅ **Change Review**
- See exactly what changed before accepting
- Changes synced to host only on exit
- `.git` directory excluded from sync
- Host project preserved until explicit sync

**Read more**: [VM_ISOLATION.md](VM_ISOLATION.md)

## Development

### Project Structure

```
safe-coder/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── config.rs         # Configuration management
│   ├── llm/              # LLM client integrations
│   │   ├── mod.rs
│   │   ├── anthropic.rs  # Anthropic API client
│   │   └── openai.rs     # OpenAI API client
│   ├── tools/            # Agent tools
│   │   ├── mod.rs
│   │   ├── read.rs       # Read file tool
│   │   ├── write.rs      # Write file tool
│   │   ├── edit.rs       # Edit file tool
│   │   └── bash.rs       # Bash execution tool
│   ├── tui/              # Terminal UI (Cyberpunk theme)
│   │   ├── mod.rs        # TUI runner
│   │   ├── app.rs        # Application state
│   │   ├── ui.rs         # UI rendering (neon colors)
│   │   ├── messages.rs   # Message types
│   │   ├── spinner.rs    # Loading animation
│   │   └── banner.rs     # ASCII banner
│   ├── vm/               # Firecracker VM management
│   │   └── mod.rs        # VM lifecycle, isolation, sync
│   ├── git/              # Git change tracking
│   │   └── mod.rs        # Auto-commit, rollback, diff
│   └── session/          # Session management
│       └── mod.rs        # Tool execution in VM
├── Cargo.toml
├── README.md
├── VM_ISOLATION.md       # Security architecture (Firecracker)
├── DOCKER_BACKEND.md     # Docker isolation backend
├── OLLAMA_SETUP.md       # Local LLM setup with Ollama
└── CYBERPUNK_THEME.md    # TUI theme documentation
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

### VM Fails to Start

- Ensure Firecracker is installed: `which firecracker`
- Verify kernel and rootfs paths in config
- Check KVM is available: `ls /dev/kvm`
- Ensure you have permissions: `sudo chmod 666 /dev/kvm` (or add user to kvm group)

### API Errors

**Cloud APIs (Anthropic/OpenAI):**
- Verify your API key is correct: `safe-coder config --show`
- Check your API key has sufficient credits
- Ensure you're using a valid model name

**Ollama:**
- Check Ollama is running: `curl http://localhost:11434/api/tags`
- Verify model is installed: `ollama list`
- Start Ollama: `ollama serve`
- See [OLLAMA_SETUP.md](OLLAMA_SETUP.md) for more help

### File Synchronization Issues

- Check disk space: `df -h`
- Verify write permissions on project directory
- Look at logs: `RUST_LOG=debug safe-coder chat`

## Limitations

### Firecracker Backend
- **Linux Only**: Requires Linux with KVM
- **x86_64/aarch64**: Supports x86_64 and ARM64 architectures
- **Resource Overhead**: Each VM uses ~512MB RAM
- **Startup Time**: VM initialization takes 1-2 seconds

### Docker Backend
- **Shared Kernel**: Weaker isolation than Firecracker (containers share host kernel)
- **Docker Required**: Must have Docker installed
- **Platform-Specific**: Docker Desktop needed for macOS/Windows

## Future Enhancements

- [x] Beautiful TUI with multi-panel layout
- [x] Real-time VM status monitoring
- [x] Loading animations and progress indicators
- [x] Cyberpunk neon theme with pulsing borders
- [x] VM isolation with strict sandbox enforcement
- [x] Git change tracking in VM
- [x] Auto-commit after tool execution
- [ ] Manual approval flow before sync back to host
- [ ] Interactive diff viewer in TUI
- [ ] Selective file sync (choose what to accept)
- [ ] Syntax highlighting for code blocks in TUI
- [ ] Support for persistent VM images per project
- [ ] Network isolation with controlled access
- [ ] Multi-project workspace support
- [ ] VS Code extension integration
- [ ] Real-time file watching and synchronization
- [ ] Custom tool definitions
- [ ] Export conversation history
- [ ] Additional theme options for TUI

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - See LICENSE file for details

## Acknowledgments

- Built with [Firecracker](https://github.com/firecracker-microvm/firecracker) for VM isolation
- TUI powered by [Ratatui](https://github.com/ratatui-org/ratatui)
- Inspired by Claude Code and similar AI coding assistants
- Uses Anthropic's Claude API for AI capabilities
- Color scheme inspired by Google CLI and Claude Code
