# Build and Plan Modes: Comprehensive Analysis & Implementation Plan

## Executive Summary

You've created a sophisticated **unified planning system** (`src/unified_planning`) but it's **not integrated** with the main commands. The current system has two parallel execution mode concepts that need to be unified:

1. **Approval-based modes** (`approval::ExecutionMode`): Plan vs Act
2. **Execution strategy modes** (`unified_planning::ExecutionMode`): Direct vs Subagent vs Orchestration

These are **orthogonal concerns** that should work together, not compete.

---

## Current State Analysis

### What Exists

#### 1. Unified Planning System (✅ Implemented, ❌ Not Integrated)

**Location**: `src/unified_planning/`

**Components**:
- ✅ `UnifiedPlanner` - Creates mode-aware plans using LLM
- ✅ `PlanRunner` - Orchestrates execution with lifecycle management
- ✅ `DirectExecutor` - Sequential inline execution
- ✅ `SubagentPlanExecutor` - Parallel execution with internal agents
- ✅ `OrchestrationExecutor` - External CLI workers in git worktrees
- ✅ `ExecutorRegistry` - Registry pattern for executor selection
- ✅ `PlanEvent` system - Event-driven progress updates

**Status**: Fully implemented with tests, but **completely unused** in actual commands.

#### 2. Approval System (✅ Used, but conflated with execution strategy)

**Location**: `src/approval/mod.rs`

**Current Usage**:
```rust
// In main.rs - run_chat()
let execution_mode = ExecutionMode::from_str(&mode)?; // "plan" or "act"
session.set_execution_mode(execution_mode);

// In main.rs - run_orchestrate()
let execution_mode = ExecutionMode::from_str(&mode)?;
config.execution_mode = execution_mode;
```

**Problem**: The `approval::ExecutionMode` is being used for TWO different purposes:
1. Whether to require approval (correct use)
2. Implicitly suggesting execution strategy (incorrect conflation)

---

### The Conceptual Gap

```
╔═══════════════════════════════════════════════════════════════╗
║                    WHAT WE HAVE NOW                           ║
╠═══════════════════════════════════════════════════════════════╣
║                                                               ║
║  approval::ExecutionMode                                      ║
║  ┌─────────────┐                                              ║
║  │ Plan Mode   │ → Shows plan, requires approval             ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ Act Mode    │ → Auto-executes                             ║
║  └─────────────┘                                              ║
║                                                               ║
║  unified_planning::ExecutionMode (UNUSED!)                    ║
║  ┌─────────────┐                                              ║
║  │ Direct      │ → Sequential inline execution               ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ Subagent    │ → Parallel internal agents                  ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ Orchestrate │ → External CLI workers                      ║
║  └─────────────┘                                              ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝

╔═══════════════════════════════════════════════════════════════╗
║                    WHAT IT SHOULD BE                          ║
╠═══════════════════════════════════════════════════════════════╣
║                                                               ║
║  User Mode (approval workflow)                                ║
║  ┌─────────────┐                                              ║
║  │ PLAN Mode   │ → Deep planning + approval required         ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ BUILD Mode  │ → Quick planning + auto-execute             ║
║  └─────────────┘                                              ║
║                    ↓                                          ║
║                    ↓ Uses                                     ║
║                    ↓                                          ║
║  Execution Strategy (how to execute)                          ║
║  ┌─────────────┐                                              ║
║  │ Direct      │ → Simple tasks, sequential                  ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ Subagent    │ → Medium tasks, internal parallelism        ║
║  └─────────────┘                                              ║
║  ┌─────────────┐                                              ║
║  │ Orchestrate │ → Complex tasks, full isolation             ║
║  └─────────────┘                                              ║
║                                                               ║
╚═══════════════════════════════════════════════════════════════╝
```

---

## Claude Code Comparison

### How Claude Code Does It

Claude Code has two distinct modes:

**PLAN Mode**:
- Read-only tools only (grep, read, list, find)
- Deep exploration before any changes
- Creates detailed execution plan
- User approves before switching to BUILD
- Focus: Understanding and planning

**BUILD Mode**:
- Full tool access (edit, write, bash)
- Executes the approved plan
- Makes actual changes
- Can still show incremental plans
- Focus: Execution and delivery

### Safe-Coder's Advantage

We can go **beyond** Claude Code by combining:
- **Plan/Build workflow** (like Claude Code)
- **+ Smart execution strategy** (Direct/Subagent/Orchestration)
- **+ Real-time progress tracking** (events, TUI)
- **+ Git safety** (checkpoints, undo/redo)

---

## Proposed Architecture

### 1. Rename and Clarify

```rust
// src/approval/mod.rs
pub enum UserMode {
    /// PLAN mode: Deep analysis, show detailed plan, require approval
    /// - LLM gets read-only tools first (optional: tool restriction)
    /// - Creates comprehensive plan with LLM
    /// - Shows plan with affected files, risks, complexity
    /// - Waits for user approval
    /// - Then executes with full tool access
    Plan,
    
    /// BUILD mode: Quick action, lightweight planning, auto-execute
    /// - LLM gets all tools immediately
    /// - Creates brief execution plan
    /// - Auto-executes without approval
    /// - Shows real-time progress
    Build,
}

// Keep ApprovalMode for per-tool approval (orthogonal concern)
pub enum ApprovalMode {
    Default,   // Ask per tool
    AutoEdit,  // Auto-approve edits
    Yolo,      // Auto-approve everything
}
```

### 2. Integration Flow

```rust
// In Session
pub struct Session {
    // ... existing fields ...
    
    // User-facing mode
    user_mode: UserMode,  // Plan or Build
    
    // Execution strategy (can be auto-detected)
    execution_strategy: Option<ExecutionStrategy>,  // Direct, Subagent, Orchestration
    
    // Unified planning components
    planner: Option<UnifiedPlanner>,
    runner: Option<PlanRunner>,
}

impl Session {
    pub async fn send_message(&mut self, input: String) -> Result<String> {
        match self.user_mode {
            UserMode::Plan => self.handle_plan_mode(input).await,
            UserMode::Build => self.handle_build_mode(input).await,
        }
    }
    
    async fn handle_plan_mode(&mut self, input: String) -> Result<String> {
        // 1. Analyze request and suggest execution strategy
        let strategy = self.suggest_strategy(&input).await?;
        
        // 2. Create plan with unified planner
        let planner = UnifiedPlanner::new(strategy);
        let plan = planner.create_plan(
            &*self.llm_client,
            &input,
            Some(&self.context_manager.build_context()?),
        ).await?;
        
        // 3. Show detailed plan to user
        let plan_display = self.format_plan_for_approval(&plan);
        self.emit_event(SessionEvent::Plan(PlanEvent::PlanAwaitingApproval {
            plan_id: plan.id.clone(),
        }));
        
        // 4. Wait for approval (this is handled by the TUI/CLI)
        // In CLI: prompt user yes/no
        // In TUI: show plan modal with approve/reject buttons
        
        // 5. If approved, execute with runner
        let runner = create_runner_with_approval(
            self.project_path.clone(),
            self.config.clone(),
        );
        
        let (completed_plan, mut events) = runner.execute(plan).await?;
        
        // 6. Stream events to UI
        while let Some(event) = events.recv().await {
            self.emit_event(SessionEvent::Plan(event));
        }
        
        Ok(completed_plan.summary())
    }
    
    async fn handle_build_mode(&mut self, input: String) -> Result<String> {
        // 1. Auto-detect strategy (or use Direct for simple)
        let strategy = self.suggest_strategy(&input).await?
            .unwrap_or(ExecutionStrategy::Direct);
        
        // 2. Create plan (still creates a plan, just lighter weight)
        let planner = UnifiedPlanner::new(strategy);
        let plan = planner.create_plan(
            &*self.llm_client,
            &input,
            Some(&self.context_manager.build_context()?),
        ).await?;
        
        // 3. Show brief summary (not full plan)
        self.emit_event(SessionEvent::Thinking(format!(
            "Executing {}-step plan...",
            plan.total_steps()
        )));
        
        // 4. Auto-execute without approval
        let runner = create_runner(
            self.project_path.clone(),
            self.config.clone(),
        );
        
        let (completed_plan, mut events) = runner.execute(plan).await?;
        
        // 5. Stream events to UI
        while let Some(event) = events.recv().await {
            self.emit_event(SessionEvent::Plan(event));
        }
        
        Ok(completed_plan.summary())
    }
    
    async fn suggest_strategy(&self, input: &str) -> Result<ExecutionStrategy> {
        // Use heuristics or ask LLM to suggest strategy
        use crate::unified_planning::suggest_execution_mode;
        
        let estimated_files = self.estimate_affected_files(input).await?;
        let has_parallel = self.detect_parallel_opportunities(input);
        
        Ok(suggest_execution_mode(input, estimated_files, has_parallel))
    }
}
```

---

## Implementation Plan

### Phase 1: Unify the Concepts (2-3 hours)

**Goal**: Clarify the two orthogonal concerns.

**Files to modify**:
1. `src/approval/mod.rs`:
   - Rename `ExecutionMode` → `UserMode` 
   - Keep values as `Plan` and `Build` (not Act)
   - Add clear documentation about the difference
   - Keep `ApprovalMode` for per-tool approval

2. `src/unified_planning/types.rs`:
   - Rename `ExecutionMode` → `ExecutionStrategy`
   - Keep values as `Direct`, `Subagent`, `Orchestration`
   - Add documentation about auto-detection

3. `src/session/mod.rs`:
   - Update to use `UserMode` for user-facing mode
   - Add `execution_strategy: Option<ExecutionStrategy>`
   - Update `set_execution_mode` → `set_user_mode`

4. `src/main.rs`:
   - Update CLI args: `--mode` takes `plan` or `build`
   - Update help text to clarify the difference

**Validation**:
```bash
cargo build  # Should compile after renames
cargo test   # All tests should pass
```

### Phase 2: Integrate Unified Planning into Chat (3-4 hours)

**Goal**: Make `chat` command use the unified planning system.

**Files to modify**:
1. `src/session/mod.rs`:
   - Add `use crate::unified_planning::*;`
   - Implement `handle_plan_mode()` and `handle_build_mode()`
   - Wire up `send_message()` to route based on `user_mode`
   - Add strategy suggestion logic

2. `src/tui/shell_app.rs` (if using TUI):
   - Add plan approval UI
   - Show detailed plan in modal
   - Add approve/reject buttons
   - Stream plan execution events

3. `src/commands/mod.rs` (if using CLI):
   - Add approval prompt for CLI mode
   - Format plan display for terminal

**Validation**:
```bash
# Test PLAN mode
cargo run -- chat --mode plan
> "add validation to the user form"
# Should show detailed plan and wait for approval

# Test BUILD mode  
cargo run -- chat --mode build
> "add a comment to main.rs"
# Should auto-execute with brief progress
```

### Phase 3: Integrate into Orchestrator (2-3 hours)

**Goal**: Make `orchestrate` command use unified planning.

**Files to modify**:
1. `src/orchestrator/mod.rs`:
   - The orchestrator IS ALREADY an executor
   - Make it work with `UnifiedPlan` instead of custom planning
   - Use `OrchestrationExecutor` from `unified_planning`

2. `src/main.rs` - `run_orchestrate()`:
   - Create `UnifiedPlanner` with `ExecutionStrategy::Orchestration`
   - Use `PlanRunner` to execute
   - Stream events to stdout

**Current Orchestrator Flow**:
```
process_request() → decompose_request() → spawn_workers()
```

**New Flow**:
```
process_request() → UnifiedPlanner.create_plan() → PlanRunner.execute()
   → OrchestrationExecutor.execute_steps()
   → spawn workers from step.suggested_executor
```

**Validation**:
```bash
# Test orchestrate with PLAN mode
cargo run -- orchestrate --mode plan
> "refactor the authentication system"
# Should create multi-worker plan and ask approval

# Test orchestrate with BUILD mode
cargo run -- orchestrate --mode build  
> "add tests to all modules"
# Should auto-execute with worker delegation
```

### Phase 4: Enhance Executors (2-3 hours)

**Goal**: Connect executor placeholders to real implementations.

**Files to modify**:
1. `src/unified_planning/executors/direct.rs`:
   - Remove placeholder `StepResultBuilder::success()`
   - Actually call session's tool execution
   - Pass step instructions to LLM
   - Execute returned tool calls
   - Return actual results

2. `src/unified_planning/executors/subagent.rs`:
   - Import `SubagentExecutor` from `src/subagent/executor.rs`
   - Create subagent scope from step
   - Run subagent and collect output
   - Return actual results

3. `src/unified_planning/executors/orchestration.rs`:
   - Import `WorkspaceManager` from `src/orchestrator/workspace.rs`
   - Import `Worker` from `src/orchestrator/worker.rs`
   - Create actual worktrees
   - Spawn external CLI workers
   - Collect and merge results

**Validation**:
```bash
# Test Direct execution
cargo run -- chat --mode build
> "fix typo in README"
# Should execute inline without spawning processes

# Test Subagent execution
cargo run -- chat --mode build
> "add tests for the auth module"
# Should spawn internal subagent (check logs)

# Test Orchestration execution
cargo run -- orchestrate --mode build --task "update all dependencies"
# Should create worktrees and spawn external workers
```

### Phase 5: Polish & Documentation (1-2 hours)

**Goal**: Make it user-friendly and well-documented.

**Tasks**:
1. Update README.md with mode explanations
2. Add examples of when to use each mode
3. Update `--help` text with clear guidance
4. Add TUI mode indicator (show "PLAN" or "BUILD" in header)
5. Add keyboard shortcut to toggle modes (e.g., Ctrl+M)
6. Write migration guide from old `act`/`plan` to new system

**Example README section**:
````markdown
## Build and Plan Modes

Safe-Coder offers two execution modes inspired by Claude Code:

### PLAN Mode (`--mode plan`)
**When to use**: Complex refactoring, multi-file changes, uncertain requirements

- Creates detailed execution plan with LLM
- Shows affected files, risks, and complexity
- Requires explicit approval before execution
- Can auto-detect best execution strategy (Direct/Subagent/Orchestration)

```bash
safe-coder chat --mode plan
> "refactor the authentication system to use JWT"
# Shows comprehensive plan → You approve → Executes
```

### BUILD Mode (`--mode build`) - Default
**When to use**: Quick fixes, well-defined tasks, trusted environments

- Creates lightweight plan internally
- Auto-executes without approval
- Shows real-time progress
- Still uses smart execution strategies

```bash
safe-coder chat --mode build
> "add a TODO comment in main.rs"
# Executes immediately with progress updates
```

### Execution Strategies (Auto-detected)

The planner automatically chooses:
- **Direct**: Simple single-file changes → executes inline
- **Subagent**: Medium complexity → spawns specialized internal agents
- **Orchestration**: Large multi-module tasks → external CLI workers in isolated worktrees

You can override with `--strategy` flag:
```bash
safe-coder chat --mode build --strategy orchestration
```
````

---

## File Changes Summary

| File | Changes | Estimated Time |
|------|---------|---------------|
| `src/approval/mod.rs` | Rename ExecutionMode → UserMode, update docs | 30 min |
| `src/unified_planning/types.rs` | Rename ExecutionMode → ExecutionStrategy | 15 min |
| `src/session/mod.rs` | Integrate unified planning, add mode handlers | 3 hours |
| `src/main.rs` | Update CLI args, call new session methods | 1 hour |
| `src/tui/shell_app.rs` | Add plan approval UI, event streaming | 2 hours |
| `src/commands/mod.rs` | Add CLI approval prompt | 30 min |
| `src/orchestrator/mod.rs` | Integrate with unified planning | 2 hours |
| `src/unified_planning/executors/direct.rs` | Connect to session tools | 1 hour |
| `src/unified_planning/executors/subagent.rs` | Connect to SubagentExecutor | 1 hour |
| `src/unified_planning/executors/orchestration.rs` | Connect to Workspace & Worker | 1.5 hours |
| `README.md` | Document modes and strategies | 1 hour |
| **Total** | | **13-14 hours** |

---

## Benefits After Implementation

### For Users
- ✅ Clear mental model: "Do I want to review (PLAN) or just do it (BUILD)?"
- ✅ Smart execution: System picks best strategy automatically
- ✅ Safety: PLAN mode for risky operations, BUILD mode for quick tasks
- ✅ Visibility: Real-time progress tracking in TUI
- ✅ Control: Can override strategy if needed

### For Developers
- ✅ Clean separation of concerns (approval vs execution)
- ✅ Extensible: Easy to add new execution strategies
- ✅ Testable: Each executor is independently testable
- ✅ Event-driven: UI updates through event streams
- ✅ Reusable: Same system works for chat, orchestrate, and future commands

### Compared to Claude Code
- ✅ **Equal**: Plan/Build workflow
- ✅ **Better**: Multiple execution strategies (Claude Code only has one)
- ✅ **Better**: Real-time progress events
- ✅ **Better**: Git safety with checkpoints and undo
- ✅ **Better**: TUI with rich visualization
- ✅ **Better**: Multi-provider orchestration (Claude, Gemini, Copilot)

---

## Quick Start Commands (After Implementation)

```bash
# Install and setup
cargo install --path .
safe-coder init

# PLAN mode for careful work
safe-coder chat --mode plan
> "refactor the error handling to use anyhow"

# BUILD mode for quick tasks (default)
safe-coder chat --mode build  # or just: safe-coder chat
> "add type hints to calculate() function"

# Orchestrate with multiple workers
safe-coder orchestrate --mode plan --task "add integration tests to all modules"

# Override execution strategy
safe-coder chat --mode build --strategy orchestration
> "update all dependencies and fix breaking changes"

# TUI mode with mode toggle
safe-coder shell --ai
# Press Ctrl+M to toggle between PLAN and BUILD modes
```

---

## Open Questions

1. **Strategy Override**: Should users be able to override the auto-detected strategy?
   - **Recommendation**: Yes, add `--strategy` flag but make it optional

2. **Mode Toggle**: Should TUI allow toggling between PLAN/BUILD mid-session?
   - **Recommendation**: Yes, add Ctrl+M keybinding

3. **Mixed Mode**: Should we support per-message mode selection?
   - **Recommendation**: Not initially - can add later as `/plan` and `/build` commands

4. **Progress Verbosity**: Should BUILD mode be quieter than PLAN mode?
   - **Recommendation**: Yes - BUILD shows brief progress, PLAN shows detailed steps

5. **Default Mode**: Should default be BUILD or PLAN?
   - **Recommendation**: BUILD (like Claude Code) - safe default for most users

---

## Migration Path

For existing users:

### Before (Current)
```bash
safe-coder chat --mode act   # Auto-execute
safe-coder chat --mode plan  # Ask approval
```

### After (New)
```bash
safe-coder chat --mode build # Auto-execute (new name)
safe-coder chat --mode plan  # Ask approval (same name)

# Alias support for backward compatibility
safe-coder chat --mode act   # Alias to 'build'
```

### Config Migration
```toml
# Old config.toml
[session]
execution_mode = "act"  # or "plan"

# New config.toml
[session]
user_mode = "build"  # or "plan"
# Optional: override strategy
# execution_strategy = "direct"  # or "subagent" or "orchestration"
```

---

## Testing Strategy

### Unit Tests
- ✅ UserMode parsing and display
- ✅ ExecutionStrategy selection logic
- ✅ Each executor independently
- ✅ Plan runner lifecycle
- ✅ Event emission

### Integration Tests
```rust
#[tokio::test]
async fn test_plan_mode_requires_approval() {
    let session = create_test_session(UserMode::Plan).await;
    let result = session.send_message("add validation").await;
    // Assert plan was created and approval was requested
}

#[tokio::test]
async fn test_build_mode_auto_executes() {
    let session = create_test_session(UserMode::Build).await;
    let result = session.send_message("fix typo").await;
    // Assert execution happened without approval
}

#[tokio::test]
async fn test_strategy_auto_detection() {
    // Simple task → Direct
    let strategy = suggest_execution_mode("fix typo", 1, false);
    assert_eq!(strategy, ExecutionStrategy::Direct);
    
    // Complex task → Orchestration
    let strategy = suggest_execution_mode("refactor auth", 10, true);
    assert_eq!(strategy, ExecutionStrategy::Orchestration);
}
```

### Manual Testing Scenarios
1. PLAN mode with approval flow
2. BUILD mode auto-execution
3. Strategy auto-detection accuracy
4. TUI mode toggle
5. Orchestration with multiple workers
6. Error handling and recovery
7. Event streaming performance

---

## Success Criteria

Implementation is complete when:

1. ✅ `cargo build` compiles without errors
2. ✅ All existing tests pass
3. ✅ New integration tests pass
4. ✅ PLAN mode shows plan and waits for approval
5. ✅ BUILD mode auto-executes without approval
6. ✅ Strategy is auto-detected correctly
7. ✅ Direct executor runs inline
8. ✅ Subagent executor spawns internal agents
9. ✅ Orchestration executor creates worktrees
10. ✅ TUI displays mode and allows toggling
11. ✅ Events stream to UI in real-time
12. ✅ Documentation is complete and accurate

---

## Next Steps

**Immediate Actions**:
1. Review this document and confirm approach
2. Start with Phase 1 (rename and clarify)
3. Validate with `cargo test`
4. Continue with Phase 2 (integrate into chat)
5. Test manually before moving to Phase 3

**Questions to Answer**:
- Do you want to keep backward compatibility with `--mode act`?
- Should we add a mode indicator in the TUI header?
- Do you want a `/mode` command to change mode mid-session?
- Should orchestrate command auto-default to Orchestration strategy?

**Once complete**, Safe-Coder will have a best-in-class planning and execution system that combines the clarity of Claude Code's Plan/Build workflow with the power of multi-strategy execution and real-time progress tracking.