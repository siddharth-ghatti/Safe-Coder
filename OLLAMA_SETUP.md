# 🦙 Using Ollama with Safe Coder

## Overview

Safe Coder supports **Ollama** for running AI models **locally** on your machine. No API keys required, no cloud dependencies, complete privacy!

## Why Ollama?

### ✅ **Benefits**
- **100% Local**: All processing happens on your machine
- **Free**: No API costs or usage limits
- **Private**: Your code never leaves your computer
- **Fast**: No network latency (for local models)
- **Offline**: Works without internet connection

### ⚠️ **Trade-offs**
- Requires local compute resources (RAM, GPU optional)
- Model quality depends on your hardware
- Smaller models may be less capable than Claude/GPT-4

## Installation

### 1. Install Ollama

**macOS/Linux:**
```bash
curl -fsSL https://ollama.com/install.sh | sh
```

**Windows:**
Download from https://ollama.com/download

**Verify installation:**
```bash
ollama --version
```

### 2. Pull a Model

Ollama supports many models. Here are some good options for coding:

**Recommended for coding:**
```bash
# DeepSeek Coder - Excellent for code (6.7B, ~4GB)
ollama pull deepseek-coder:6.7b-instruct

# Qwen Coder - Fast and capable (7B, ~4.7GB)
ollama pull qwen2.5-coder:7b-instruct

# CodeLlama - Meta's code model (13B, ~7.3GB)
ollama pull codellama:13b-instruct

# Llama 3.1 - General purpose, good at code (8B, ~4.7GB)
ollama pull llama3.1:8b-instruct
```

**For more capable systems (16GB+ RAM):**
```bash
# Qwen Coder 32B - Very capable (32B, ~19GB)
ollama pull qwen2.5-coder:32b-instruct

# DeepSeek Coder V2 - Latest version (16B, ~9GB)
ollama pull deepseek-coder-v2:16b-instruct
```

**List available models:**
```bash
ollama list
```

### 3. Start Ollama Server

Ollama runs as a background service:

```bash
# Start the Ollama server
ollama serve

# Or just run a model (auto-starts server)
ollama run deepseek-coder:6.7b-instruct
```

The server runs at `http://localhost:11434` by default.

## Configuration

### Basic Setup

Edit `~/.config/safe-coder/config.toml`:

```toml
[llm]
provider = "ollama"
model = "deepseek-coder:6.7b-instruct"
max_tokens = 8192

# API key not needed for Ollama
# api_key = ""

# Optional: Custom Ollama URL (defaults to http://localhost:11434)
# base_url = "http://localhost:11434"
```

### Example Configurations

**DeepSeek Coder (Recommended):**
```toml
[llm]
provider = "ollama"
model = "deepseek-coder:6.7b-instruct"
max_tokens = 8192
```

**Qwen Coder (Fast & Capable):**
```toml
[llm]
provider = "ollama"
model = "qwen2.5-coder:7b-instruct"
max_tokens = 8192
```

**CodeLlama (Larger):**
```toml
[llm]
provider = "ollama"
model = "codellama:13b-instruct"
max_tokens = 4096
```

**Custom Ollama Server:**
```toml
[llm]
provider = "ollama"
model = "deepseek-coder:6.7b-instruct"
max_tokens = 8192
base_url = "http://192.168.1.100:11434"  # Remote Ollama server
```

## Usage

Once configured, use Safe Coder normally:

```bash
# Start a coding session with Ollama
./safe-coder chat --path /your/project

# Output:
🦙 Using Ollama (local LLM)
🐳 Auto-selected Docker (darwin detected)
🐳 Creating isolated copy of project in Docker container
...
```

## Model Recommendations

### For Coding Tasks

| Model | Size | RAM Needed | Quality | Speed | Best For |
|-------|------|------------|---------|-------|----------|
| **deepseek-coder:6.7b-instruct** | 4GB | 8GB | ⭐⭐⭐⭐ | Fast | General coding |
| **qwen2.5-coder:7b-instruct** | 4.7GB | 8GB | ⭐⭐⭐⭐ | Very Fast | Quick tasks |
| **codellama:13b-instruct** | 7.3GB | 16GB | ⭐⭐⭐⭐ | Medium | Complex code |
| **qwen2.5-coder:32b-instruct** | 19GB | 32GB | ⭐⭐⭐⭐⭐ | Slow | Best quality |
| **deepseek-coder-v2:16b-instruct** | 9GB | 16GB | ⭐⭐⭐⭐⭐ | Medium | Latest tech |

### Hardware Requirements

**Minimum (8GB RAM):**
- Model: `deepseek-coder:6.7b-instruct` or `qwen2.5-coder:7b-instruct`
- Performance: Adequate for most tasks
- Speed: 5-15 tokens/sec (CPU)

**Recommended (16GB RAM):**
- Model: `codellama:13b-instruct` or `deepseek-coder-v2:16b-instruct`
- Performance: Good for complex tasks
- Speed: 10-30 tokens/sec (CPU)

**Optimal (32GB+ RAM + GPU):**
- Model: `qwen2.5-coder:32b-instruct`
- Performance: Excellent, near GPT-4 quality
- Speed: 50+ tokens/sec (with GPU)

### GPU Acceleration

Ollama automatically uses GPU if available:

**NVIDIA GPU (Linux/Windows):**
- Automatically detected
- 10-50x faster than CPU
- Check: `nvidia-smi`

**Apple Silicon (macOS):**
- Metal acceleration enabled by default
- 5-20x faster than CPU
- Works on M1/M2/M3

**AMD GPU (Linux):**
- ROCm support available
- Check Ollama docs for setup

## Performance Tips

### 1. Choose the Right Model Size

```bash
# Fast but less capable (good for simple tasks)
ollama pull qwen2.5-coder:7b-instruct

# Balanced (recommended for most users)
ollama pull deepseek-coder:6.7b-instruct

# Slow but very capable (if you have the hardware)
ollama pull qwen2.5-coder:32b-instruct
```

### 2. Adjust max_tokens

```toml
[llm]
max_tokens = 4096  # Faster, shorter responses
# max_tokens = 8192  # Slower, longer responses
```

### 3. Use Quantized Models

```bash
# Q4 quantization (faster, less memory, slightly lower quality)
ollama pull deepseek-coder:6.7b-instruct-q4_0

# Q8 quantization (balanced)
ollama pull deepseek-coder:6.7b-instruct-q8_0
```

### 4. Pre-load Models

```bash
# Keep model loaded in memory
ollama run deepseek-coder:6.7b-instruct
# (keep this terminal open)
```

## Troubleshooting

### Ollama Not Running

```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama
ollama serve

# Or on macOS (if installed via app)
# Ollama should auto-start from Applications
```

### Model Not Found

```bash
# List installed models
ollama list

# Pull the model you need
ollama pull deepseek-coder:6.7b-instruct
```

### Out of Memory

```bash
# Use a smaller model
ollama pull qwen2.5-coder:3b-instruct

# Or use quantized version
ollama pull deepseek-coder:6.7b-instruct-q4_0
```

### Slow Performance

```bash
# Check if GPU is being used (NVIDIA)
nvidia-smi

# Check if GPU is being used (macOS)
# Activity Monitor > GPU History

# Use a smaller/faster model
ollama pull qwen2.5-coder:7b-instruct
```

### Connection Refused

```bash
# Check Ollama is running
ollama list

# Check the port
lsof -i :11434

# Try starting manually
ollama serve
```

## Comparison: Ollama vs Cloud APIs

| Feature | Ollama | Claude/GPT-4 |
|---------|--------|--------------|
| **Cost** | Free | $0.01-0.10 per request |
| **Privacy** | 100% local | Data sent to cloud |
| **Speed** | Depends on hardware | Usually fast |
| **Quality** | Good (smaller models) | Excellent |
| **Offline** | ✅ Works offline | ❌ Requires internet |
| **Setup** | Install + download model | Just API key |
| **RAM Usage** | 4-32GB | None (cloud) |

## Best Practices

### 1. Start with a Smaller Model

```bash
# Try this first
ollama pull deepseek-coder:6.7b-instruct
```

Test it out, then upgrade if needed.

### 2. Keep Ollama Running

```bash
# Start Ollama in background
ollama serve &

# Or keep a terminal with ollama run open
```

### 3. Monitor Resource Usage

```bash
# Check memory usage
htop  # or top

# Check GPU usage (NVIDIA)
nvidia-smi

# macOS: Activity Monitor
```

### 4. Use Appropriate Context Length

```toml
[llm]
max_tokens = 4096  # Good for most tasks
# max_tokens = 8192  # For large files/complex tasks
```

## Advanced Configuration

### Remote Ollama Server

Run Ollama on a powerful server, access from laptop:

**On server:**
```bash
# Start Ollama with network access
OLLAMA_HOST=0.0.0.0:11434 ollama serve
```

**On client (Safe Coder config):**
```toml
[llm]
provider = "ollama"
model = "qwen2.5-coder:32b-instruct"
base_url = "http://server-ip:11434"
```

### Multiple Models

Switch between models by editing config:

```bash
# Edit config
vim ~/.config/safe-coder/config.toml

# Change model name
[llm]
model = "deepseek-coder:6.7b-instruct"  # or another model
```

Or create multiple config files and switch:

```bash
cp ~/.config/safe-coder/config.toml ~/.config/safe-coder/config-ollama-small.toml
cp ~/.config/safe-coder/config.toml ~/.config/safe-coder/config-ollama-large.toml

# Edit each with different model
# Then symlink the one you want to use
```

## Popular Models for Coding

### Top Recommendations

**1. DeepSeek Coder 6.7B** (Best balanced)
```bash
ollama pull deepseek-coder:6.7b-instruct
```
- Excellent code understanding
- Fast on most hardware
- Good at explaining code

**2. Qwen 2.5 Coder 7B** (Fastest)
```bash
ollama pull qwen2.5-coder:7b-instruct
```
- Very fast inference
- Good code generation
- Works well on 8GB RAM

**3. CodeLlama 13B** (Higher quality)
```bash
ollama pull codellama:13b-instruct
```
- Meta's flagship code model
- Better reasoning
- Needs 16GB RAM

**4. Qwen 2.5 Coder 32B** (Best quality)
```bash
ollama pull qwen2.5-coder:32b-instruct
```
- Near GPT-4 quality on code
- Excellent reasoning
- Needs 32GB RAM + GPU

## Getting Help

### Ollama Resources
- Website: https://ollama.com
- Models: https://ollama.com/library
- GitHub: https://github.com/ollama/ollama
- Discord: https://discord.gg/ollama

### Safe Coder with Ollama
- Check Ollama is running: `ollama list`
- Verify model is installed: `ollama list | grep deepseek`
- Test model directly: `ollama run deepseek-coder:6.7b-instruct "write a hello world"`
- Check Safe Coder config: `cat ~/.config/safe-coder/config.toml`

## Summary

Ollama lets you run Safe Coder **completely locally**:
- ✅ No API costs
- ✅ Complete privacy
- ✅ Works offline
- ✅ Same tool features (read, write, edit, bash)
- ✅ Same isolation (Firecracker/Docker)
- ⚠️ Requires decent hardware (8GB+ RAM)
- ⚠️ Smaller models less capable than GPT-4

**Quick Start:**
```bash
# 1. Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# 2. Pull a model
ollama pull deepseek-coder:6.7b-instruct

# 3. Configure Safe Coder
cat >> ~/.config/safe-coder/config.toml << 'EOF'
[llm]
provider = "ollama"
model = "deepseek-coder:6.7b-instruct"
max_tokens = 8192
EOF

# 4. Run Safe Coder
./safe-coder chat --path /your/project
```

Enjoy coding with complete privacy! 🦙
