# 🐳 Docker Backend for Safe Coder

## Overview

Safe Coder supports **two isolation backends**:

1. **Firecracker** (Linux only) - Maximum security with hardware-level VM isolation
2. **Docker** (All platforms) - Cross-platform container isolation

## Why Docker Backend?

### ✅ **Cross-Platform**
- Works on **Linux**, **macOS**, and **Windows**
- No need for KVM or hardware virtualization
- Easy setup with Docker Desktop

### ✅ **Lighter Weight**
- ~100MB memory overhead vs ~512MB for Firecracker
- Faster startup (~100ms vs 1-2 seconds)
- Better for resource-constrained environments

### ✅ **Development Friendly**
- Great for local development on macOS/Windows
- Easier debugging and troubleshooting
- Familiar Docker ecosystem

### ⚠️ **Security Trade-Off**
- **Shared kernel** with host (weaker isolation than Firecracker)
- Container escapes are possible (though rare)
- **Still provides good isolation** for most use cases

## Platform Support

| Platform | Firecracker | Docker | Default |
|----------|-------------|--------|---------|
| **Linux** | ✅ Supported | ✅ Supported | Firecracker |
| **macOS** | ❌ Not available | ✅ Supported | Docker |
| **Windows** | ❌ Not available | ✅ Supported | Docker |

## Installation

### Linux

```bash
# Install Docker
curl -fsSL https://get.docker.com | sh

# Add your user to docker group
sudo usermod -aG docker $USER

# Log out and back in, then verify
docker --version
```

### macOS

1. Download Docker Desktop from https://www.docker.com/products/docker-desktop
2. Install and start Docker Desktop
3. Verify:
   ```bash
   docker --version
   ```

### Windows

1. Download Docker Desktop from https://www.docker.com/products/docker-desktop
2. Install and start Docker Desktop
3. Verify in PowerShell:
   ```powershell
   docker --version
   ```

## Configuration

### Auto-Selection (Recommended)

By default, Safe Coder automatically selects the best backend:

```toml
[isolation]
backend = "auto"  # Firecracker on Linux, Docker elsewhere
```

### Force Docker

To always use Docker (even on Linux):

```toml
[isolation]
backend = "docker"
```

### Docker Settings

Customize Docker container settings:

```toml
[docker]
image = "ubuntu:22.04"      # Base image to use
cpus = 2.0                  # CPU limit (number of cores)
memory_mb = 512             # Memory limit in MB
auto_pull = true            # Auto-pull image if not present
```

## How It Works

### 1. **Initialization**

When you start a session, Safe Coder:

```
1. Checks if Docker image exists (pulls if needed)
2. Creates temp sandbox: /tmp/safe-coder-{uuid}/
3. Copies your project to sandbox
4. Initializes git repository in sandbox
5. Creates Docker container with:
   - Volume mount to sandbox
   - CPU and memory limits
   - No network access (--network none)
   - Working directory: /workspace
```

### 2. **Agent Operations**

All tool executions happen in the Docker container sandbox:

```
- read_file: Reads from /tmp/safe-coder-{uuid}/
- write_file: Writes to /tmp/safe-coder-{uuid}/
- edit_file: Edits in /tmp/safe-coder-{uuid}/
- bash: Executes in /tmp/safe-coder-{uuid}/
```

After each tool execution:
```
- Changes auto-committed to git
- Commit message: "Agent executed: tool1, tool2"
```

### 3. **Cleanup**

When you exit:

```
1. Get change summary from git
2. Display changes to user
3. Sync files back to host (excluding .git)
4. Stop and remove Docker container
5. Clean up temp sandbox
```

## Example Session

```bash
# On macOS or Windows (auto-selects Docker)
./safe-coder chat --path /your/project

# Output:
🐳 Auto-selected Docker (darwin detected)
🐳 Creating isolated copy of project in Docker container
✓ Project copied to container sandbox: /tmp/safe-coder-abc123
✓ Git tracking initialized in container
Created Docker container: 4f3a2b1c
Started Docker container with ID: 4f3a2b1c
🐳 Docker container isolation active - agent confined to sandbox
```

## Security Features

Despite using containers, Safe Coder maintains strong security:

### 🔒 **Namespace Isolation**
- Separate filesystem view
- Isolated process table
- Network disabled (`--network none`)

### 📝 **Git Change Tracking**
- All changes committed automatically
- Full audit trail
- Rollback support

### 🛡️ **Resource Limits**
- CPU limits prevent abuse
- Memory limits prevent exhaustion
- No access to Docker daemon

### ✅ **Safe Sync**
- Changes reviewed before sync
- `.git` directory excluded
- Host project preserved until sync

## Docker vs Firecracker Comparison

| Feature | Docker | Firecracker |
|---------|--------|-------------|
| **Platforms** | Linux, macOS, Windows | Linux only |
| **Kernel Isolation** | ❌ Shared kernel | ✅ Separate kernel |
| **Memory Overhead** | ~100MB | ~512MB |
| **Startup Time** | ~100ms | 1-2 seconds |
| **Setup Complexity** | Low | High |
| **Escape Difficulty** | Medium | Extremely Hard |
| **Development Use** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Production Use** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

## Recommendations

### Use Docker When:
- ✅ Developing on macOS or Windows
- ✅ Resource-constrained environment
- ✅ Need fast iteration cycles
- ✅ Docker already installed
- ✅ Development/testing workloads

### Use Firecracker When:
- ✅ Running on Linux servers
- ✅ Maximum security required
- ✅ Production deployments
- ✅ Untrusted code execution
- ✅ CI/CD pipelines (Linux)

## Troubleshooting

### Docker Not Found

```bash
# Verify Docker is installed
docker --version

# If not found, install Docker Desktop (macOS/Windows)
# or use package manager (Linux)
```

### Permission Denied

```bash
# Linux: Add user to docker group
sudo usermod -aG docker $USER

# Then log out and back in
```

### Image Pull Fails

```bash
# Manually pull the image
docker pull ubuntu:22.04

# Or use a different image in config
[docker]
image = "alpine:latest"
```

### Container Won't Start

```bash
# Check Docker is running
docker ps

# Check Docker Desktop is running (macOS/Windows)
# Look for whale icon in system tray
```

### Out of Disk Space

```bash
# Clean up old containers and images
docker system prune -a

# Check disk usage
docker system df
```

## Advanced Configuration

### Custom Docker Image

You can use a custom Docker image with pre-installed tools:

```toml
[docker]
image = "your-custom-image:latest"
auto_pull = false  # Don't pull, use local image
```

Example Dockerfile for custom image:

```dockerfile
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    git \
    curl \
    build-essential \
    python3 \
    nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace
```

Build and use:

```bash
docker build -t safe-coder-custom .

# Update config.toml
[docker]
image = "safe-coder-custom:latest"
```

### Resource Limits

Adjust based on your workload:

```toml
[docker]
cpus = 4.0        # More CPU for compute-heavy tasks
memory_mb = 2048  # More memory for large projects
```

### Network Access

By default, containers have **no network** access for security.

To enable (NOT recommended unless necessary):

Modify `src/isolation/docker.rs`:
```rust
// Remove this line:
"--network", "none",

// Or use a custom network:
"--network", "bridge",
```

## Benefits for Development

### 🚀 **Fast Iteration**
- Lightweight containers start quickly
- No VM overhead
- Faster project switching

### 🔧 **Easy Debugging**
- Familiar Docker tools
- Can exec into container
- Inspect volumes directly

### 💻 **Works Everywhere**
- Same experience on all platforms
- No special hardware requirements
- Easy CI/CD integration

### 🧪 **Great for Testing**
- Quick setup for testing
- Easy to reproduce issues
- Portable across environments

## Future Enhancements

- [ ] Support for docker-compose multi-container setups
- [ ] Custom network policies for controlled access
- [ ] Volume caching for faster startups
- [ ] Integration with Docker buildx for multi-arch
- [ ] Support for podman as Docker alternative
- [ ] Dev containers integration

## Summary

The Docker backend provides a **cross-platform, lightweight** isolation solution that:
- ✅ Works on macOS and Windows (where Firecracker doesn't)
- ✅ Provides good isolation for development use
- ✅ Maintains git tracking and safety features
- ✅ Auto-pulls and configures containers
- ⚠️ Has weaker isolation than Firecracker (shared kernel)

For **maximum security on Linux**, use **Firecracker**.
For **cross-platform development**, use **Docker**.

Both backends provide the same git tracking, change review, and safe sync features!
