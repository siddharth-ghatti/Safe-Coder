# 🔒 VM ISOLATION & GIT TRACKING

## Overview

Safe Coder implements **strict VM isolation** to ensure the AI agent operates in a secure sandbox, protecting your host system from potentially dangerous operations.

## Key Features

### 🔒 **Isolated Execution Environment**

When you start Safe Coder:

1. **Project Copy**: Your entire project is copied to a temporary VM sandbox directory
2. **VM-Only Operations**: ALL agent tools (read, write, edit, bash) execute ONLY within the VM
3. **No Host Access**: The agent has zero access to your host filesystem during operation
4. **Git Tracking**: Every change is tracked with git inside the VM

### 📝 **Automatic Git Tracking**

Every operation is version-controlled:

```
🔒 VM Sandbox at: /tmp/safe-coder-{uuid}/

Initial State:
├── .git/                    # Auto-initialized
└── [your project files]     # Copied from host

After Tool Execution:
├── .git/
│   └── commits/            # Auto-commits after each tool
├── file.rs                 # Modified by agent
└── new_file.rs            # Created by agent
```

### 🔄 **Sync Back to Host**

When the VM stops, changes are synced back:

```rust
// Changes detected in VM:
// - Modified: src/main.rs
// - Created: tests/new_test.rs
// - Deleted: old_file.rs
//
// Syncing to host...
// ✓ Changes synced to host: /your/project
```

**Important**: The `.git` directory from the VM is **excluded** when syncing back, so your host's git history is preserved.

## Architecture

### VM Lifecycle

```
┌─────────────────────────────────────────────────┐
│ 1. INITIALIZATION                               │
│    - Create temp sandbox: /tmp/safe-coder-{id}/ │
│    - Copy project files to sandbox              │
│    - Initialize git repository in sandbox       │
│    - Initial commit: "Initial snapshot"         │
└─────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────┐
│ 2. AGENT OPERATIONS (in VM only)                │
│    - Tool execution: read, write, edit, bash    │
│    - Working dir: /tmp/safe-coder-{id}/         │
│    - Auto-commit after each tool execution      │
│    - Commit message: "Agent executed: tool1,..."│
└─────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────┐
│ 3. SHUTDOWN                                     │
│    - Get change summary from git                │
│    - Display changes to user                    │
│    - Sync files back (excluding .git)           │
│    - Cleanup VM sandbox                         │
└─────────────────────────────────────────────────┘
```

### Security Guarantees

✅ **Agent cannot access host filesystem**
   - Tool execution requires VM to be running
   - No fallback to host paths
   - Strict isolation enforced at session level

✅ **All changes are tracked**
   - Git initialized automatically
   - Auto-commit after every tool use
   - Full audit trail of agent actions

✅ **Changes require sync back**
   - Explicit sync operation on VM stop
   - User sees what changed before sync
   - Original project preserved until sync

✅ **VM cleanup**
   - Temporary sandbox deleted on exit
   - No persistent VM state
   - Fresh environment each session

## Implementation Details

### Code Structure

**`src/vm/mod.rs`** - VM Management
```rust
pub struct VmManager {
    config: VmConfig,
    instance: Option<VmInstance>,
}

pub struct VmInstance {
    pub id: Uuid,
    pub socket_path: PathBuf,
    pub project_path: PathBuf,    // Original host path
    pub shared_dir: PathBuf,       // VM sandbox path
    pub git_manager: GitManager,   // Git tracking
    process: Option<Child>,
}
```

**`src/git/mod.rs`** - Git Tracking
```rust
pub struct GitManager {
    repo_path: PathBuf,  // Points to VM sandbox
}

impl GitManager {
    pub async fn init_if_needed(&self) -> Result<()>
    pub async fn auto_commit(&self, message: &str) -> Result<()>
    pub async fn get_change_summary(&self) -> Result<ChangeSummary>
    pub async fn snapshot(&self, label: &str) -> Result<()>
    pub async fn rollback(&self) -> Result<()>
}
```

**`src/session/mod.rs`** - Tool Execution
```rust
// 🔒 SECURITY: Require VM to be running
let working_dir = self.vm_manager
    .get_shared_dir()
    .context("VM not running - tool execution requires active VM")?;

// Execute tool in VM sandbox
tool.execute(input, working_dir).await?;

// 🔒 Auto-commit changes after tool execution
let commit_message = format!("Agent executed: {}", tools_executed.join(", "));
self.vm_manager.commit_changes(&commit_message).await?;
```

### File Operations

**Copying to VM**:
```rust
// Copy entire project to VM sandbox
self.copy_dir_all(&project_path, &shared_dir)?;
```

**Syncing back to Host**:
```rust
// Copy files back, excluding .git directory
self.copy_dir_all_excluding(&instance.shared_dir, &instance.project_path, &[".git"])?;
```

## Git Commit Messages

The VM automatically creates commits with these patterns:

```
Initial snapshot - Safe Coder VM          # On VM start
Agent executed: read, write               # After tool use
🔒 Snapshot: before-major-operation       # Manual snapshots
```

## Change Summary

Before syncing back to host, you'll see:

```
🔒 Changes detected in VM:
Changed 3 file(s):
  - src/main.rs
  - tests/test_new.rs
  - README.md
Syncing to host...
✓ Changes synced to host: /your/project
```

## Safety Features

### Rollback Support

```rust
// Rollback to previous commit (in VM)
git_manager.rollback().await?;
```

### Manual Snapshots

```rust
// Create checkpoint before risky operations
git_manager.snapshot("before-refactor").await?;
```

### Change Review

```rust
// Get detailed diff before syncing
let changes = git_manager.get_change_summary().await?;
println!("{}", changes.summary_text());
```

## Usage

### Start a Session (VM Auto-Starts)

```bash
./target/release/safe-coder chat --path /your/project
```

**What happens**:
1. VM sandbox created at `/tmp/safe-coder-{uuid}/`
2. Project copied to sandbox
3. Git initialized with initial commit
4. Agent operates entirely within sandbox

### During Session

All tools execute in VM:
```
User: "Create a new file called hello.rs"
🔒 Executing tool in VM sandbox: write
✓ Auto-committed: Agent executed: write
```

### End Session

```
User: "exit"

🔒 Syncing VM changes back to host...
Changed 1 file(s):
  - hello.rs
✓ Changes synced to host: /your/project
✓ Stopped VM d4e2b8c1-4f3a-4b5e-8e9f-1a2b3c4d5e6f
```

## Demo Mode

Test without Firecracker:

```bash
./target/release/safe-coder chat --demo --path .
```

Demo mode simulates VM isolation using temp directories but doesn't require Firecracker setup.

## Benefits

🛡️ **Security**: Agent can't accidentally damage host system
📊 **Auditability**: Full git history of all agent actions
🔄 **Reversibility**: Can rollback any changes in VM
🧪 **Testing**: Safe environment for experimental code
🔍 **Transparency**: See exactly what changed before accepting

## Future Enhancements

- [ ] Manual approval before sync back to host
- [ ] Selective file sync (choose what to accept)
- [ ] Diff viewer in TUI before accepting changes
- [ ] Network isolation in Firecracker VM
- [ ] Resource limits (CPU, memory, disk)
- [ ] Multiple VM instances for parallel tasks
