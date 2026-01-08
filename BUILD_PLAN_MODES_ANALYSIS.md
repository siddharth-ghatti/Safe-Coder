# Build and Plan Modes: Analysis and Integration Plan

## Executive Summary

After reviewing the codebase, I've identified that **two separate mode systems exist but are not integrated**:

1. **Approval Mode System** (`approval::ExecutionMode`): Plan vs Act - controls approval workflow
2. **Unified Planning System** (`unified_planning::ExecutionMode`): Direct vs Subagent vs Orchestration - controls execution strategy

**The Problem**: The unified planning system is fully implemented but **not being used** in the actual chat/orchestrate commands. The old approval system is still active.

**The Solution**: These are orthogonal concerns that should work together:
- **Plan Mode** = Unified planning WITH approval before execution
- **Build Mode** = Unified planning WITHOUT approval (auto-execute)

---

## Current State

### 1. Approval Mode System (Currently Active)

**Location**: `src/approval/mod.rs`

```rust
pub enum ExecutionMode {
    Plan,  // Deep planning with user approval before execution
    Act,   // Lighter planning that auto-executes
}
```

**Used By**:
- `run_chat()` in `main.rs` - Sets mode via `session.set_execution_mode()`
- `run_orchestrate()` in `main.rs` - Passes mode to orchestrator config
- Commands accept `--mode plan` or `--mode act` flags

**What It Does**:
- Controls whether `ExecutionPlan` is shown to user for approval
- Determines if tools auto-execute or need confirmation
- Works with `ApprovalMode` enum (Plan/Default/AutoEdit/Yolo)

### 2. Unified Planning System (Implemented But Not Used)

**Location**: `src/unified_planning/`

**Components**:
- ✅ `UnifiedPlanner` - Creates mode-aware plans using LLM
- ✅ `PlanRunner` - Orchestrates plan execution lifecycle
- ✅ `DirectExecutor` - Sequential inline execution
- ✅ `SubagentPlanExecutor` - Parallel subagent execution
- ✅ `OrchestrationExecutor` - External CLI workers in worktrees
- ✅ `ExecutorRegistry` - Maps modes to executors

**Execution Modes**:
```rust
pub enum ExecutionMode {
    Direct,        // Inline, sequential, no parallelism
    Subagent,      // Internal agents, parallel within process
    Orchestration, // External workers, full isolation
}
```

**What It Provides**:
- LLM-generated plans with step groups and dependencies
- Mode-aware planning (LLM knows execution capabilities)
- Parallel execution where supported
- Event stream for UI updates
- Executor abstraction for different strategies

**Status**: 
- ✅ All core types implemented (`types.rs`)
- ✅ Planner complete with mode-specific prompts (`planner.rs`)
- ✅ Runner complete with approval support (`runner.rs`)
- ✅ All three executors implemented (placeholder logic)
- ❌ **NOT integrated into chat or orchestrate commands**
- ❌ **NOT wired up to Session**
- ❌ **Executors have TODO comments for actual integration**

---

## The Gap

### What's Missing

1. **No Integration in Commands**
   - `run_chat()` doesn't use unified planning
   - `run_orchestrate()` doesn't use unified planning
   - Session still uses old message-based execution

2. **Executor Placeholders**
   - `DirectExecutor` returns placeholder output, doesn't use Session tools
   - `SubagentPlanExecutor` doesn't connect to actual SubagentExecutor
   - `OrchestrationExecutor` doesn't use WorkspaceManager or Worker

3. **Mode Confusion**
   - Two different `ExecutionMode` enums with same name
   - Old system: `approval::ExecutionMode`
   - New system: `unified_planning::ExecutionMode`
   - They serve different purposes but naming is confusing

4. **No Bridge Between Systems**
   - Approval system controls when to execute
   - Unified planning controls how to execute
   - Need to combine: Plan mode should use unified planning WITH approval

---

## Proposed Solution

### Clarify the Modes

#### Rename for Clarity

**Old System (Approval)**:
```rust
// Rename to avoid confusion
pub enum ApprovalMode {
    Plan,  // Require approval before execution
    Build, // Auto-execute without approval
}
```

**New System (Execution Strategy)**:
```rust
// Keep as-is in unified_planning
pub enum ExecutionMode {
    Direct,        // Sequential inline
    Subagent,      // Parallel internal agents
    Orchestration, // Parallel external workers
}
```

#### How They Work Together

| User Mode | Approval | Execution Strategy | Behavior |
|-----------|----------|-------------------|----------|
| `--mode plan` | Required | Auto-detect or specify | Show plan → Wait for approval → Execute with strategy |
| `--mode build` | None | Auto-detect or specify | Generate plan → Auto-execute with strategy |

**Examples**:
```bash
# Plan mode with auto-detected strategy
safe-coder chat --mode plan

# Plan mode with explicit orchestration
safe-coder chat --mode plan --execution orchestration

# Build mode with subagents
safe-coder chat --mode build --execution subagent

# Orchestrate command (always uses orchestration strategy)
safe-coder orchestrate --mode plan --task "refactor auth"
```

### Integration Architecture

```
User Request
    │
    ▼
┌─────────────────────────────────────┐
│  Command Handler (chat/orchestrate) │
│  - Parse approval mode (plan/build) │
│  - Parse/detect execution mode      │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  UnifiedPlanner.create_plan()       │
│  - Mode-aware prompts               │
│  - LLM generates plan with groups   │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  PlanRunner.execute()               │
│  - If plan mode: wait for approval  │
│  - If build mode: auto-execute      │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Executor (Direct/Subagent/Orch)    │
│  - Execute steps per strategy       │
│  - Emit events for UI               │
└─────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Naming Cleanup (30 minutes)

**Goal**: Eliminate confusion between the two ExecutionMode enums.

1. **Rename approval mode**:
   ```rust
   // src/approval/mod.rs
   pub enum ApprovalWorkflow {
       Plan,  // Require approval
       Build, // Auto-execute
   }
   ```

2. **Update imports**:
   - `src/main.rs` - Update command handlers
   - `src/session/mod.rs` - Update field name
   - `src/orchestrator/mod.rs` - Update config

3. **Update CLI help text**:
   ```rust
   /// Execution mode: plan (create plan and wait for approval) or build (auto-execute)
   #[arg(short, long, default_value = "build")]
   mode: String,
   ```

### Phase 2: Wire Up Direct Executor (1-2 hours)

**Goal**: Make DirectExecutor actually execute steps using Session's tool registry.

**Changes to `src/unified_planning/executors/direct.rs`**:

```rust
pub struct DirectExecutor {
    // Add session reference (or tool registry)
    session: Arc<Mutex<Session>>,
}

async fn execute_step(&self, step: &UnifiedStep, ...) -> Result<StepResult> {
    let timer = StepTimer::start();
    ctx.emit_step_started(group_id, step);
    
    // Get session and send instructions as a message
    let mut session = self.session.lock().await;
    let response = session.send_message(step.instructions.clone()).await?;
    
    // Parse response for tool usage and results
    let files_modified = session.get_modified_files_since_last_checkpoint();
    
    let result = StepResultBuilder::success()
        .with_output(response)
        .with_duration(timer.elapsed_ms())
        .with_files(files_modified)
        .build();
    
    ctx.emit_step_completed(&step.id, &result);
    Ok(result)
}
```

### Phase 3: Wire Up Subagent Executor (1-2 hours)

**Goal**: Connect to existing SubagentExecutor.

**Changes to `src/unified_planning/executors/subagent.rs`**:

```rust
use crate::subagent::{SubagentExecutor, SubagentScope, SubagentConfig};

async fn execute_with_subagent(...) -> Result<StepResult> {
    let timer = StepTimer::start();
    ctx.emit_step_started(group_id, step);
    
    // Create subagent scope from step
    let scope = SubagentScope::new(
        kind.to_string(),
        step.instructions.clone(),
        step.relevant_files.clone(),
    );
    
    // Create subagent executor
    let config = SubagentConfig::default();
    let mut executor = SubagentExecutor::new(
        ctx.config.clone(),
        ctx.project_path.clone(),
        scope,
        config,
        kind,
    ).await?;
    
    // Run subagent
    let result = executor.run().await?;
    
    let step_result = StepResultBuilder::from_bool(result.success)
        .with_output(result.summary)
        .with_duration(timer.elapsed_ms())
        .with_files(result.files_modified)
        .build();
    
    ctx.emit_step_completed(&step.id, &step_result);
    Ok(step_result)
}
```

### Phase 4: Wire Up Orchestration Executor (2-3 hours)

**Goal**: Connect to existing WorkspaceManager and Worker.

**Changes to `src/unified_planning/executors/orchestration.rs`**:

```rust
use crate::orchestrator::{WorkspaceManager, Worker, WorkerConfig, WorkerKind};

pub struct OrchestrationExecutor {
    max_concurrent: usize,
    workspace_manager: Arc<Mutex<WorkspaceManager>>,
    worker_configs: HashMap<WorkerKind, WorkerConfig>,
}

async fn create_workspace(&self, step_id: &str, ctx: &ExecutorContext) -> Result<PathBuf> {
    let mut ws_manager = self.workspace_manager.lock().await;
    let workspace = ws_manager.create_workspace(step_id).await?;
    Ok(workspace.path.clone())
}

async fn execute_with_worker(...) -> Result<StepResult> {
    let timer = StepTimer::start();
    ctx.emit_step_started(group_id, step);
    
    // Create workspace
    let workspace_path = self.create_workspace(&step.id, ctx).await?;
    
    // Get worker config
    let worker_config = self.worker_configs.get(&kind)
        .cloned()
        .unwrap_or_default();
    
    // Create and run worker
    let mut worker = Worker::new(
        step.id.clone(),
        kind,
        workspace_path,
        worker_config,
    );
    
    let output = worker.execute(&step.instructions).await?;
    
    let result = StepResultBuilder::from_bool(output.success)
        .with_output(output.stdout)
        .with_duration(timer.elapsed_ms())
        .with_files(step.relevant_files.clone())
        .build();
    
    ctx.emit_step_completed(&step.id, &result);
    Ok(result)
}
```

### Phase 5: Integrate into Chat Command (2-3 hours)

**Goal**: Replace old execution flow with unified planning.

**Changes to `src/main.rs::run_chat()`**:

```rust
async fn run_chat(
    project_path: PathBuf,
    use_tui: bool,
    demo: bool,
    mode: String,
    execution: Option<String>, // NEW: optional execution strategy
) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;
    
    // Parse approval workflow
    let approval_workflow = ApprovalWorkflow::from_str(&mode)?;
    
    // Parse or detect execution mode
    let execution_mode = if let Some(exec) = execution {
        ExecutionMode::from_str(&exec)?
    } else {
        ExecutionMode::Direct // Default for chat
    };
    
    let config = Arc::new(Config::load()?);
    
    // In TUI mode
    if use_tui {
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.set_approval_workflow(approval_workflow);
        tui_runner.set_execution_mode(execution_mode);
        tui_runner.initialize().await?;
        tui_runner.run_unified().await?; // NEW: Use unified planning
        return Ok(());
    }
    
    // CLI mode with unified planning
    let session = Session::new(config.clone(), canonical_path.clone()).await?;
    let llm_client = session.llm_client(); // Need to expose this
    
    println!("🤖 Safe Coder - AI Coding Assistant");
    println!("Mode: {} | Execution: {:?}", mode, execution_mode);
    println!("Type your request or 'exit' to quit\n");
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input == "exit" { break; }
        
        // Use unified planning
        let require_approval = approval_workflow.requires_approval();
        let (plan, mut events) = plan_and_execute(
            input,
            execution_mode,
            llm_client.as_ref(),
            canonical_path.clone(),
            config.clone(),
            None, // project context
            require_approval,
        ).await?;
        
        // Stream events
        while let Some(event) = events.recv().await {
            print_event(&event);
        }
        
        println!("\n✨ {}", plan.summary());
    }
    
    Ok(())
}
```

### Phase 6: Integrate into Orchestrate Command (1-2 hours)

**Goal**: Use unified planning for orchestration mode.

**Changes to `src/main.rs::run_orchestrate()`**:

```rust
async fn run_orchestrate(...) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;
    let config = Arc::new(Config::load()?);
    
    // Parse approval workflow
    let approval_workflow = ApprovalWorkflow::from_str(&mode)?;
    
    // Orchestrate command ALWAYS uses Orchestration execution mode
    let execution_mode = ExecutionMode::Orchestration;
    
    // Create LLM client
    let llm_client = create_client(&config)?;
    
    if let Some(task_text) = task {
        // One-shot mode
        let require_approval = approval_workflow.requires_approval();
        let (plan, mut events) = plan_and_execute(
            &task_text,
            execution_mode,
            llm_client.as_ref(),
            canonical_path.clone(),
            config.clone(),
            None,
            require_approval,
        ).await?;
        
        // Print events
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                print_orchestration_event(&event);
            }
        });
        
        println!("\n{}", plan.summary());
        return Ok(());
    }
    
    // Interactive mode...
}
```

### Phase 7: TUI Integration (2-3 hours)

**Goal**: Update TUI to show unified planning progress.

**Changes to `src/tui/shell_app.rs`**:

1. Add plan/build mode toggle
2. Show execution strategy selector
3. Display plan steps with progress
4. Handle PlanEvent stream
5. Show approval UI for plan mode

**New UI Components**:
```
┌─────────────────────────────────────────────────────┐
│ Mode: [Plan] Build          Exec: Direct ▼          │
├─────────────────────────────────────────────────────┤
│                                                      │
│ 📋 Plan: Refactor authentication                    │
│                                                      │
│ Group 1: Preparation                                │
│   ✓ Read auth.rs                [completed 2.1s]    │
│   ✓ Analyze dependencies        [completed 1.8s]    │
│                                                      │
│ Group 2: Implementation (parallel)                  │
│   ⏳ Update auth module          [in progress...]   │
│   ⏳ Add tests                   [in progress...]   │
│   ⏸ Update documentation        [pending...]        │
│                                                      │
│ Progress: 2/5 steps complete                        │
│                                                      │
│ [Approve] [Reject] [Edit Plan]                      │
└─────────────────────────────────────────────────────┘
```

---

## Testing Strategy

### Unit Tests (Already Exist)

- ✅ `UnifiedPlanner` parsing and prompt building
- ✅ `PlanRunner` lifecycle
- ✅ Each executor's basic functionality

### Integration Tests (Need to Add)

1. **End-to-end plan execution**:
   ```rust
   #[tokio::test]
   async fn test_plan_mode_with_approval() {
       // Create session
       // Send request
       // Verify plan created
       // Approve plan
       // Verify execution
   }
   ```

2. **Build mode auto-execution**:
   ```rust
   #[tokio::test]
   async fn test_build_mode_auto_execute() {
       // Create session
       // Send request
       // Verify plan created AND executed without approval
   }
   ```

3. **Different execution modes**:
   ```rust
   #[tokio::test]
   async fn test_direct_vs_subagent_vs_orchestration() {
       // Same task, different execution modes
       // Verify appropriate executor used
   }
   ```

---

## Migration Path

### For Existing Users

**Current Commands**:
```bash
safe-coder chat --mode plan    # Still works
safe-coder chat --mode act     # Renamed to --mode build
```

**New Commands**:
```bash
safe-coder chat --mode plan                # Plan mode, auto-detect execution
safe-coder chat --mode build               # Build mode, auto-detect execution
safe-coder chat --mode plan --exec subagent # Plan mode, force subagent execution
```

### Config File Changes

**Add to `config.toml`**:
```toml
[execution]
default_mode = "build"              # plan or build
default_strategy = "auto"           # auto, direct, subagent, orchestration
max_concurrent_subagents = 3
max_concurrent_workers = 3

[execution.orchestration]
# Existing orchestrator config stays here
```

---

## Benefits After Integration

### 1. Clear Mode Separation
- **Plan mode**: Think before acting, review before executing
- **Build mode**: Fast iteration, auto-execute with safety nets

### 2. Execution Strategy Options
- **Direct**: Simple tasks, quick fixes
- **Subagent**: Medium complexity, focused subtasks
- **Orchestration**: Large refactors, parallel modules

### 3. Mode-Aware Planning
- LLM knows execution capabilities upfront
- Plans optimized for the execution strategy
- Parallel grouping when supported

### 4. Better UX
- Clear progress indication
- Parallel execution visibility
- Approval at plan level, not per-tool
- Rollback entire plan if needed

### 5. Claude Code Similarity
- Plan mode ≈ Claude Code "plan and execute"
- Build mode ≈ Claude Code "build mode"
- Both use structured planning approach

---

## Timeline Estimate

| Phase | Time | Priority |
|-------|------|----------|
| 1. Naming cleanup | 30m | HIGH |
| 2. Direct executor | 1-2h | HIGH |
| 3. Subagent executor | 1-2h | HIGH |
| 4. Orchestration executor | 2-3h | MEDIUM |
| 5. Chat integration | 2-3h | HIGH |
| 6. Orchestrate integration | 1-2h | MEDIUM |
| 7. TUI integration | 2-3h | HIGH |
| **Total** | **10-16h** | |

**Recommended Order**:
1. Phase 1 (naming) - Foundation
2. Phase 2 (direct) - Core functionality
3. Phase 5 (chat) - Make it usable
4. Phase 7 (TUI) - Polish UX
5. Phase 3 (subagent) - Add parallelism
6. Phase 6 (orchestrate) - Complete integration
7. Phase 4 (orchestration) - Full feature set

---

## Open Questions

1. **Auto-detection of execution mode**: What heuristics?
   - File count, complexity, keywords?
   - Or always require explicit flag?

2. **Approval granularity in plan mode**:
   - Approve entire plan only?
   - Or allow per-step approval?
   - Or per-group approval?

3. **Fallback strategy**:
   - If orchestration fails, fall back to subagent?
   - Or fail the entire plan?

4. **Plan editing**:
   - Allow users to modify LLM-generated plans?
   - Add/remove steps, change executors?

5. **Checkpointing**:
   - Checkpoint before plan execution?
   - Per-step checkpoints?
   - Only on failure?

---

## Conclusion

The unified planning system is **well-architected and complete**, but currently exists in isolation. The integration work is straightforward:

1. Clarify naming (approval workflow vs execution strategy)
2. Wire up executor implementations to existing components
3. Replace old execution flow in commands
4. Update TUI to visualize plans

After integration, Safe-Coder will have:
- ✅ Clear plan/build modes like Claude Code
- ✅ Flexible execution strategies (direct/subagent/orchestration)
- ✅ Mode-aware LLM planning
- ✅ Parallel execution where supported
- ✅ Better progress visibility
- ✅ Approval at the right granularity

The refactor is mostly **plumbing work** - connecting well-designed components that already exist.