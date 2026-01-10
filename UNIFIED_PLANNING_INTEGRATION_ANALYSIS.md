# Unified Planning Integration Analysis

## Executive Summary

You've built an Modes** (`approval::ExecutionMode`): Plan excellent **unified planning system** (`src/unified_planning/`) that provides vs Act - controls approval workflow
2. **Execution Modes** (`unified_planning::ExecutionMode`): Direct vs Subagent vs centralized planning and execution across three modes Orchestration - controls execution strategy (Direct, Subagent, Orchestration). However, **it

**These are orthogonal concerns that should work together!**

---'s not currently integrated** with the CLI commands. The old `approval::ExecutionMode` (

## Current State Assessment

### ✅ What's Already BuiltPlan/Act) system is still in use.

This document analyzes the current state and provides a clear

The unified planning system is **fully implemented** but unused: integration path to achieve Claude Code-style

```
src/unified_planning/
├── types.rs              ✅ Core types "build" and "plan" modes.

---

## Current State

### Two Separate (UnifiedPlan, StepGroup, UnifiedStep)
├── planner.rs            ✅ L Systems

#### 1. **Approval System** (`src/approval/mod.rs`)
```rust
pub enumLM-based planning with mode-aware prompts
├── runner.rs             ✅ Plan ExecutionMode {
    Plan,  // Deep planning with user approval before execution lifecycle management
├── integration.rs        ✅ Helper functions for integration
├── executor execution
    Act,   // Lighter planning_trait.rs     ✅ PlanExecutor trait and registry
└ that auto-executes
}
```

**Used by**:── executors/
    ├── direct.rs         ✅ Sequential inline execution
    ├── subagent `run_chat()` and `run_orchestrate()` in `main.rs`

**Purpose.rs       ✅ Parallel internal agents (TODO:**: Controls whether user approval is required before tool execution

** wire to actual subagents)
    └── orchestration.rs  ✅ ExternalStatus**: ✅ Fully integrated with Session CLI workers (TODO: wire to actual orchestrator)
```

**Key Features**:
- Mode-aware planning (LLM knows execution and Orchestrator

#### 2. **Unified Planning System** (`src/unified_planning/`)
```rust
pub enum ExecutionMode {
    Direct,        // Sequential execution in current session
    Subagent,      // Parallel execution with capabilities upfront)
- Step grouping with dependency tracking
- Parallel execution support
- Event streaming for UI updates
- Approval internal agents
    Orchestration, // Parallel execution with external CLI workers
}
```

**Components**:
- workflow integration
- Executor registry pattern ✅ `UnifiedPlanner` - Creates mode-aware plans using

### ❌ What's Not Integrated

The unified planning system is **completely disconnected** from the actual commands:

| LLM
- ✅ `PlanRunner` - Orchestrates plan execution lifecycle
- ✅ `Direct File | Current State | Issue |
|------|---------------|-------|
| `src/main.rs` | Uses `approval::ExecutionMode` (Executor` - Executes steps inline (placeholder implementation)
- ✅ `SubagentPPlan/Act) | Doesn't use unified planning at all |
| `src/session/lanExecutor` - Delegates to subagents (placeholder implementation)
- ✅ `OrmochestrationExecutor` -d.rs` | Has Delegates to external workers (placeholder implementation)

**Status**: `execution_mode: ExecutionMode` field | Approval mode, not execution strategy ⚠️ **Not integrated** - Never calle |
| `run_chat()` | Creates `Sessiond by any CLI command

---

## The Problem

### What You`, calls `send_message()` | No planning Want
- **Build mode**: Like Claude Code - auto-execute with appropriate parallelization, just tool-call loop
- **Plan mode**: Like Claude Code - show plan, get approval, then execute |
| `run_orchestrate()` | Uses `Orchestrator::process_request()` | Has own planning,

### What You Built
A sophisticated unified planning system that:
1. Creates ignores unified system |

**The unified planning code is orph mode-aware execution plans using LLM
2. Supports three execution strategies (Direct/Subagent/Orchestration)
3. Emits events for UIaned - nothing calls it!**

---

## Architecture Analysis updates
4. Handles approval workflow

### The Gap
The unified planning system exists but is

### Two Different "Modes" That Need to **disconnected** from the actual commands: Work Together

#### 1. Approval Mode (User-Facing)
```rust
// src/approval

```rust
// main.rs run_chat() - Still uses old approval system
let/mod.rs
pub enum ExecutionMode {
    Plan,  // Show plan execution_mode = ExecutionMode::from, require_str(&mode)?;  // approval approval before execution
    Act,   // Auto-execute with minimal approval
}::ExecutionMode
session.set_execution_mode(execution_mode);
// ❌ Never calls
```

**What it controls**: Whether UnifiedPlanner or plan_and_execute()

// main.rs run_orchestrate() - to ask user for approval before executing

#### 2. Execution Strategy (Internal)
```rust
// src Still uses old approval system
let execution_mode = ExecutionMode::from_str(&mode)?;  // approval::ExecutionMode
config.execution_mode = execution_mode;/unified_planning/types.rs
pub enum ExecutionMode {
    Direct,        // Sequential execution in current session
    Subagent,      // Parallel execution with internal agents
    Orchestration, //
// ❌ Never calls UnifiedPlanner, uses legacy Orchestrator directly
```

---

## Architecture Analysis

### These Parallel execution with external CLI workers
}
```

**What it controls**: How to execute the work Are NOT Competing Systems

The two (sequential vs parallel, inline vs isolated)

### The Confusion

**Current code conflates these two concerns `ExecutionMode` enums serve **different purposes**:

| Concern | System:**
- CLI flags use "plan | Purpose |
|---------|--------|---------|
| **Approval Flow** | `approval::ExecutionMode` | Should we ask user before" and "act" modes
- But unified planning has "Direct", executing? |
| **Execution "Subagent", " Strategy** | `unified_planning::ExecutionMode` | How should we execute (inline/Orchestration" modes
- They're both named `ExecutionMode` but meansub different things!

### The Solution

**Theseagent/workers should be separate, orthogonal settings:**

```rust
// User)? |

These are **orthogonal concerns** that should work together!

### Correct Integration Model-facing approval behavior
pub enum ApprovalMode {
    Plan,  // Requires approval (like Claude Code's "

```
User Request
    │
    ▼plan" mode)
    Build, // Auto-execute (
┌─────────────────────────────────┐
│  Determine Approval Mode        │
│  - Plan:like Claude Code's "build" mode)
}

// Internal execution strategy (could be auto-detecte Requires approval      │
│  -d or explicit)
pub enum ExecutionStrategy Build: Auto-execute          │
└───────────────────────────── {
    Direct,        // Simple tasks, sequential
    Subagent,      // Medium tasks, parallel agents────┘
    │
    ▼
┌─────────────────────────────────┐
│  Determine Execution Strategy   │
│  - Direct: Simple
    Orchestration, // Complex tasks, isolated workers
}
```

** tasksCombined usage:**
- `--mode         │
│  - Subagent: Medium complexity  │
│  - Orchestration: Large plan` → `Ap tasks   │
└─────────────provalMode::Plan────────────────────┘` +
    │
    ▼ auto-detect strategy
- `--mode build` → `ApprovalMode::Build
┌─────────────────────────────────┐
│  UnifiedPlanner.create_plan()   │
│  (` + auto-detect strategy
-mode `--strategy-aware planning)          │
└ direct` → Override auto-detection

---

## Integration Gaps

### Gap 1: Session─────────────────────────────────┘
    │
    ├── Plan Mode Doesn't Use Unified Planning

**Current flow ──► Show plan → Get approval** (`run_chat` → `session.send_message(
    │
    └── Build Mode ─► Auto-procee)`):
```
User input
    ↓
Send
    │
    ▼
┌─────────────────────────────────┐
│  PlanRunner.execute()d to LLM with all tools
    ↓
LLM responds with tool calls
    ↓
Execute tools one by one
    ↓
Sen           │
│  (uses appropriate executor)    │
└─────────────────d results back to LLM
    ↓
Repeat until done
```

**No planning step!────────────────┘
```

---

## Integration Plan

### Phase 1: Rename an** The session just reactively executes whatever tools the LLM asks for.

**Whatd Clarify (30 min)

**Goal**: Eliminate's naming confusion

1. **Rename `approval::Execut missing**:
- No upfront task decomposition
- No parallel execution
- No stepionMode`** → `approval::ApprovalMode`
   ```rust
   pub enum ApprovalMode {
       Plan, grouping
- Unified planning system unused

### Gap 2: Orchestrator Has Own Planning System

**Current flow** (`run_orchestrate` → `orchest  // Requires approval (like Claude Code "plan" mode)
       Builrator.process_request()`):
```rust
// src/orchestrator/planner.rs
pub structd, // Auto-execute (like Claude Code "build" mode)
   }
   ```

2. **Keep `unified_planning::ExecutionMode`** as TaskPlan {
    pub title: String,
    pub steps: Vec<PlanStep>,
    pub dependencies: HashMap-is
   ```rust
   pub enum ExecutionMode {
       Direct,
       Subagent,
       Orchestration,
   }
   ```

3. **Update all references**:
   - `src<String, Vec<String>>,
}
```

**The orchestrator has its own planning!** It doesn/approval/mod.rs` - Rename enum
   - `src/session/mod.rs` - Update't use the unified system either.

**Result**: Two separate planning implementations field name
   - `src/orchestrator/mod.rs` - Update field name
   - `src/main.rs` - Update imports an:
- `src/orchestrator/planner.rs` - Used usage

### Phase 2: Wire Up Unified Planning ind by orchestrate command
- `src/unified_planning/planner.rs` - Not used by anything

### Gap 3: Executors Are Placeholder Chat Command (1-2 hours)

**Goal**: Make Stubs

All three executors have TODO comments:

```rust
// src/unified_planning/executors/direct.rs
// TODO: Integrate with session tool execution `safe-coder chat` use the unified planning system

**File**: `src/main.rs` - `run_chat()`

```rust
async fn run_chat(
    project_path: PathBuf, 
    use_tui: bool, 
    demo: bool, 
    mode: String
) -> Result<()> {

// src/unified_planning/executors/subagent.rs
// TODO: Integrate with actual SubagentExecutor from src/subagent/

// src/unified_planning/executors/
    use approval::ApprovalMode;  // Renamed from ExecutionMode
    
    let canonicalorchestration.rs
// TODO: Integrate with Worker from src/orchestrator/worker.rs
```

**They return placeholder results_path = project_path.canonicalize()?;
    let config = Arc::new(Config::load()?);
    
    // instead of doing real work!**

### Gap 4: No CLI Support for Execution Parse approval mode
    let approval_mode = ApprovalMode::from_str(&mode)?;
    
    if Strategy

```rust
// src/main.rs
Chat {
    // ...
    /// use_tui {
        // TUI mode - pass approval mode to TUI
        let mut Execution mode: plan (deep planning tui_runner = tui::TuiRunner::new(canonical_path. with approval) or act (auto-execute)
    #[arg(short, long,display().to_string());
        tui_runner.initialize().await?;
        tui_runner.run_ default_value = "act")]
    mode: String,
}
```

Users can only choose "plan" or "act" - no way to specify Directwith_approval_mode(approval_mode).await?;
        return Ok(());
    }
    
    // CLI mode - create session with unified planning
    let mut session = Session::new(config./Subagent/Orchestration!

---

## Proposed Solution: Full Integration

### Phaseclone(), canonical_path.clone()).await?;
    session.set_approval_mode(approval_mode);
    
    println!(" 1: Naming Cleanup (30 minutes)

**Rename to🤖 Safe Coder - AI avoid confusion:**

```rust
// src/approval/mod.rs
pub enum Ap CodingprovalMode {  // Was: ExecutionMode
    Plan,  // Requires approval
    Build, // Auto-execute ( Assistant");
    println!("Mode: {}", if approval_mode.requires_approval() {rename from "Act" to match Claude 
        "PLAN (show plan before execution)" 
    } else { Code)
}

// src/unified_planning/types.rs
pub enum ExecutionStrategy {  // Was: ExecutionMode 
        "BUILD (auto-execute)" 
    });
    
    // Interactive loop
    loop {
        print!("\n
    Direct,
    Subagent,
    Orchestration,
}
```

**Update CLI:**
```rust
Chat> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(& {
    /// Approval mode: plan (requires approval) ormut input)?;
        let input = input.trim();
        
        if input.is_empty() { continue; }
        if input == build (auto-execute)
    #[arg(short, long, default_value = "build")]
    mode: String,  // plan "exit" { break; }
        
        // Use unified planning system
        let execution_mode = session | build
    
    /// Execution strategy: auto, direct, subagent, orchestration
    #[arg(long.suggest_execution_mode(&input)?;
        
        match plan_and_execute(
            input, default_value = "auto")]
    strategy: String,
}
```

### Phase 2: Wire Up Direct,
            execution_mode,
            session.llm_client(),
            canonical_path.clone(),
            config.clone(),
            None, // project context
            approval_mode.requires_approval(),
        ).await {
            Ok((plan, mut Executor (1-2 hours)

**Goal**: Make `DirectExecutor` actually execute steps using the session.

```rust
// src/unified_planning/executors/direct.rs
pub events)) => {
                // Stream events to console
                while let Some(event) = events.recv().await {
                    display struct DirectExecutor {
    session: Arc<Mutex<Session>>,  // Add session reference
}

impl DirectExecutor {
    async fn execute_step(&self, step: &UnifiedStep, .._plan_event(&event);
                }
                println!("\n✅ Complete.) -> Result<StepResult> {
        let mut session = self.session.lock().d: {}", plan.summary());
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);await;
        
        // Send step instructions to LLM
        let response = session.send_message_internal
            }
        }
    }
    
    Ok(())
}
```

**New Session Method**:
```rust
// src/session/mod.rs
impl Session {
    ///(&step.instructions).await?;
        
        // Return actual result
        StepResult {
            success: !response.is_empty(),
            output: response,
            // ...
        }
    }
} Suggest execution mode based on request complexity
    pub fn suggest_execution_mode(&self, request: &str) -> Result<unified_planning::ExecutionMode> {
```

**Files to modify:**
- `src/unified_planning/executors/direct.rs` - Add session integration
- `src/session
        // Use heuristics or ask LLM
        let estimate/mod.rs` - Add `send_message_internal()` for executd_files = self.estimate_affected_files(request)?;
        let has_parallel_workors

### Phase 3: Wire Up Subagent Executor (2-3 hours)

**Goal**: Make `SubagentPlanExecutor` actually spawn = request.contains("multiple") || request.contains("all");
        
        Ok(unified_planning::suggest_execution_mode(
            request,
            estimate subagents.

```rust
// src/unified_planning/executors/subagent.rs
impl SubagentPlanExecutor {
    async fn execute_withd_files,
            has_parallel_work,
        ))
    }
}
```

### Phase 3: Complete Executor_subagent(&self, step: &UnifiedStep, ...) -> Result<StepResult> {
        // Create subagent scope
        let scope = Sub Implementations (2-4 hours)

**Goal**: Make executors actually executeagentScope {
            task: step.instructions.clone(),
            relevant_files: step.relevant_files.clone instead of returning placeholders

#### 3A. DirectExecutor Integration

**File**: `src/unified_planning/executors/direct.rs`

```rust
pub struct DirectExecutor {
    session: Arc<Mutex<Session>>,  // Reference to session
}

impl DirectExecutor {
    pub fn new(session: Arc<Mutex<Session>>) -> Self {
        Self { session }
    }
}(),
            // ...
        };
        
        // Spawn actual subagent
        let executor = SubagentExecutor::new(
            kind,
            scope,
            self.config.clone(),
            // ...
        );
        
        let result = executor.run().await?;
        
        // Convert to StepResult
        StepResult {
            success: result.success,
            output: result

#.summary[async_trait]
impl PlanExecutor for DirectExecutor {
    async fn execute_step(
        &self,
        step: &UnifiedStep,
        group,
            // ...
        }
    }
}
```

**Files to modify:**
- `src/unified_planning/executors/subagent.rs` - Wire_id: &str,
        ctx: &ExecutorContext,
    ) -> Result<StepResult> {
        let timer = StepTimer::start();
        ctx.emit_step_started(group_id, step);
        
        // Actually send to LLM and execute
        let mut to `src/subagent/executor.rs`
- `src/subagent/executor.rs` - May need small API adjustments

### Phase 4: Wire Up Orchestration Executor (2-3 hours)

**Goal**: Make `OrchestrationExecutor` actually spawn workers.

```rust
// src/ session = self.session.lock().awaitunified_planning/executors/orchestration.rs
impl OrchestrationExecutor {
    async fn execute_with_worker(&self, step: &Unifie;
        let response = session.send_message_with_context(
            &step.instructions,
            &step.relevant_filesdStep, ...) -> Result<StepResult> {
        // Create workspace
        let workspace = self,
        ).await?;
        
        // Extract modified files from session
        let modified_files = session.workspace_manager.create_workspace(&step.id).await?;
        
        // Spawn worker
        let worker = Worker.get_last_modified_files();
        
        let result = StepResult::new(kind, workspace.clone(), self.config.clone());
        worker.assignBuilder::success()
            .with_output(response)_task(&step.instructions).await?;
        
        let result = worker.run().await?
            .with_duration(timer.elapsed_ms())
            .with_files(modified_files)
            .build();
        
        ctx.emit_step_completed(&step.id, &result);
        Ok(result)
    }
}
```

#### 3B. SubagentPlanExecutor Integration

**File**: `src/unified_planning/executors/subagent.rs`

```rust
use crate::subagent::{Sub;
        
        // Merge if successful
        if result.success {
            self.workspace_manager.merge_workspace(&step.id).await?;
        }agentExecutor, SubagentScope};

async fn execute_with_subagent(
    &self,
    step: &Un
        
        StepResult {
            success: result.success,
            output: result.summary,
            // ...
        }
    }
}
```

**Files to modify:**
- `src/unified_planning/executors/orchestration.rs` - Wire to `src/orchestrator/worker.rs`
- `src/orchestrator/workspace.rs` - Ensure API matches executor needs

### Phase 5: Integrate with Chat Command (2-3 hours)

**Goal**: Replace sessionifiedStep,
    group_id: &str,
    ctx: &ExecutorContext,
    kind: SubagentK's reactive tool loop with proactive planning.

**ind,
) -> Result<StepResult> {
    let timer = StepTimer::start();
    
    // Create subagent scope from step
    let scopeCurrent flow:**
```rust
async fn run_chat(mode: String, ...) {
    let execution_mode = Execut = SubagentScope::new(
        step.id.clone(),
        step.instructions.clone(),
    )
    .with_files(step.relevant_files.clone())
    .with_complexityionMode::from_str(&mode)?;
    let mut session = Session::new(...).(step.complexity_await?;
    session.set_execution_mode(execution_modescore);
    
    // Spawn subagent
    let subagent = SubagentExecutor::new(
        kind.);
    
    loop {
        // User input
        session.send_message(input).await?;to_subagent_type(),
        ctx.config  // ← Reactive tool loop.clone(),
        ctx.project_path.clone(),
        scope,
    );
    
    // Execute
    let result = subagent.execute().await?;
    }
}
```

**New flow:**
```rust
async fn run_chat(mode: String, strategy: String, ...) {
    let approval
    
    // Convert to StepResult
    Ok(StepResultBuilder::new()
        .with_success(result._mode = ApprovalMode::from_str(&mode)?;
    let execution_strategy = if strategy == "auto" {
        suggest_execution_strategy(&inputsuccess)
        .with_output(result.output)
        .with_duration(timer.elapsed_ms())
        .with_files(result.modified_files)
        .build()), &project_context)
    } else {
        ExecutionStrategy::from_str(&strategy)?
    };
    
    loop
}
```

#### 3C. OrchestrationExecutor Integration

**File**: `src/unified_planning/executors/orchestration.rs`

```rust {
        // User input
        
        // Create plan using unified planner
        let pl
use crate::orchestrator::{Worker, WorkerConfig, WorkspaceManager};

async fn execute_anner = UnifiedPlanner::new(execution_strategy);
        let plan = planner.with_worker(
    &self,
    step: &UnifiedStep,
    group_id: &str,
    ctx: &ExecutorContext,create_plan(&llm_client, &input, context).await?;
    kind: WorkerKind,
) -> Result<StepResult> {
    let timer = StepTimer::start();
    
    // Create workspace
        
        // Create runner with approval if needed
        let runner = if
    let workspace_manager = WorkspaceManager::new(ctx.project_path.clone());
    let workspace = workspace_manager.create_ approval_mode.requires_approval() {
            create_runner_with_approval(workspace(&step.id).await?;
    
    // Create worker config
    let worker_config = WorkerConfig {
        kind,
        workspace_path: workspace.pathproject_path, config)
        } else {
            create_runner(project_path, config.clone(),
        task: step.instructions.clone(),
        relevant_files: step.relevant_files)
        };
        
        // Execute plan
        let (completed_plan, events) = runner.execute(.clone(),
    };
    
    // Spawn and run worker
    let mut worker = Worker::new(worker_config);
    let result = worker.run().await?;
    
    // Mergeplan).await?;
        
        // Display results
        display_plan_results(&completed_plan, events).await?; changes if successful
    if result.success {
        workspace_manager.merge_workspace
    }
}
```

**Files to modify:**
- `src/main.rs` - Update `run_chat()` to use unifie(&step.id).await?;
    } else {
        workspace_manager.cleanupd planning
- `src/session/mod.rs` - Add method to get LLM client for planning
- `src/tui/shell__workspace(&step.id).await?;
    }
    
    Ok(StepResultBuilder::new()
        .with_success(result.success)
        .with_app.rs` - Update TUI to show planning UIoutput(result.output)
        .with_duration(timer.elapsed_ms())
        .with_files(result.modified_files)
        .build())
}
```

### Phase 4: Update Orchest

### Phase 6: Migrate Orchestrator Planningrator Command (1 hour)

**Goal**: Make `safe-coder orchestrate` use unified planning

**File**: `src/ (2-3 hours)

**Goal**: Replace orchestrator's custom planning with unified system.

**Current:**
```rust
// src/orchestrator/planner.rs -main.rs` - `run_orchestrate()`

```rust
async fn run_orchestrate(
    task: Option Custom implementation
impl Orchestrator {
    pub async fn process_request(&mut<String>,
    project_path: PathBuf,
    mode: String,
    // ... other params
) -> Result<()> {
    use approval self, request: &str) -> Result<OrchResponse> {
        let plan = self.planner.create_::ApprovalMode;
    use unified_planning::{ExecutionMode, plan_and_execute};
    
    let canonicalplan(request).await?;  // Custom planner
        self.execute_plan(plan).await?;_path = project_path.canonicalize()?;
    let config = Arc::new(Config::load()?);
    
    // Parse approval mode
    let approval_mode = ApprovalMode::
        // ...
    }
}
```

**New:**
```rust
impl Orchestrator {
    pub async fn process_request(&mut self,from_str(&mode)?;
    
    println!("🎯 Safe Coder Orchestrator");
    println!("Mode: {}", if approval_mode request: &str) -> Result<OrchResponse> {
        // Use unified planner
        let planner = Un.requires_approval() { 
        "PLAN (requires approval)" 
    } else { 
        "BUILD (auto-execute)" 
    });ifiedPlanner::new(ExecutionStrategy::Orchestration);
        let plan = planner.create_plan(&self.llm_client, request,
    
    if let Some(task_text) = task {
        // Always use Orchestration mode for orchestrate context).await?;
        
        // Use orchestration command
        let (plan, mut executor
        let runner = create_runner(self.project_path.clone(), self.config.clone()); events) = plan_and_execute(
            &task_text,
            ExecutionMode::Orchestration,
            &*create
        let (completed_plan, events) = runner.execute(plan).await?;
        
        // Convert to OrchResponse
        // ...
    }
}_client(&config)?,
            canonical_path.clone(),
            config.clone(),
            None,
            approval_mode
```

**Files to modify:**
- `src/orchestrator/mod.rs` - Switch to unified planning
- `src/orchestrator/planner.rs` - Mark.requires_approval(),
        ).await?;
        
        // Stream events
        while let Some(event) = events.recv().await {
            display_plan as deprecated or remove
- `src/main.rs` - Update `run_orchestrate()` CLI

### Phase 7: Auto_event(&event);
        }
        
        println!("\n{}", plan.summary());
    }-Detection (1-2 hours)

**Goal**: Smart strategy selection based on task
    
    Ok(())
}
```

### Phase 5: Add Mode Selection to T characteristics.

```rust
// src/unified_planning/integration.rs (alreadyUI (1-2 hours)

**Goal**: Let users toggle between Plan/Build in exists!)
pub fn suggest_execution_strategy(
    request: &str,
    project_context: &ProjectContext,
) -> Execut the TUI

**File**: `src/tui/shell_app.rs`

AdionStrategy {
    let file_count = estimate_affected_files(request, project_contextd keybinding for mode toggle:
- `Ctrl+M` - Toggle between Plan an);
    let complexity = estimate_complexity(request);
    let has_parallel_workd Build modes
- Show current mode in status bar
- Display = detect_parallel_potential(request);
    
    if file_count > 5 || complexity > 7 plan approval UI when in Plan mode

---

## Gaps to Fill

### 1. Executor Implementations (HIGH || has_parallel_work {
        ExecutionStrategy::Orchestration  // Complex, use workers
    } else if PRIORITY)

**Current State**: All three executors return file_count > 1 || complexity > 3 {
        ExecutionStrategy::Subagent  // Medium, use sub placeholder results

**What's Needed**:
- ✅ DirectExecutor -agents
    } else {
        ExecutionStrategy::Direct  // Simple, sequential
    }
}
```

**This function already Integrate with Session
- ✅ SubagentPlanExecutor - Wire up Sub exists!** Just need to call it.

---

## Timeline Estimate

|agentExecutor
- ✅ OrchestrationExecutor - Use WorkspaceManager and Worker

### 2. Approval Phase | Description | Time | Priority |
|-------|-------------|------|----------|
| 1 | Naming cleanup | 30m | HIGH |
| 2 | Direct executor integration | 1-2h | HIGH |
| 3 UI Integration (HIGH PRIORITY)

**Current State**: No UI for showing plans and getting approval

**What's Needed**:
- CLI: Pretty | Subagent executor integration | 2-3h | MEDIUM |
| 4 | Orchestration executor integration | 2-3h | MEDIUM |
| 5 | Chat-print plan with colored output
- TUI: Modal dialog showing plan with approve command integration | 2-3h | HIGH |
| 6 | Orchestrator migration | 2-3h |/reject buttons
- Both: Support MEDIUM |
| 7 | Auto-detection | 1-2h | LOW |

**Total: 11-17 hours**

** "always approve for this session" option

### 3. Execution Mode SelectionRecommended order:**
1. Phase 1 (MEDIUM PRIORITY)

**Current State**: No way to automatically choose Direct (naming) - Prevent confusion
2. Phase 2 (direct executor) - Get basic/Subagent/Orchestration

**What's Needed**:
- Heuristic function flow working
3. Phase 5 (chat integration) - Connect to user-facing comman `suggest_execution_mode()` ✅ (exists in integration.d
4. Test and iterate on basic flow
5. Phases 3,rs)
- Option to manually override: `-- 4, 6 (other executors) - Add parallel capabilities
6. Phase 7 (auto-detection) - Polish Uexecution-mode orchestration`
- LLM-based mode suggestion (X

---

## Mode Comparison: Beforeask LLM which mode is best)

### 4. Event Display (MEDIUM PRIORITY)

**Current State**: Plan vs After

### Before (Current State events emitted but not displayed properly

**What's Needed**:
- CLI)

**User perspective:**
- `safe-c: Pretty formatting for PlanEvent types
- TUI: Realoder chat --mode plan` - Shows execution plan,-time progress asks for approval
- `safe-coder chat --mode act` - Auto-executes without approval display in sidebar
- Both: Show group parallelism visually

### 5. Session Integration

**Internal behavior:**
- Both modes: Reactive tool- (MEDIUM PRIORITY)

**Current State**: Session doesn't know about unified planning

**What's Needed**:
- `Session::planby-tool execution
- No upfront planning
- No parallel execution
- No execution strategy awareness

### After (Full_and_execute()` method
- Track plan history in session
- Integrate with memory Integration)

**User perspective:**
- `safe-coder chat --mode plan` - Deep planning with approval (like Claude Code)
- `safe-coder chat --mode build system (session context → project_` - Auto-execute with planning (like Claude Code)
- `safe-coder chat --strategy direct` - Force sequential executioncontext)

---

## Recommended Implementation Order

### Week 1:
- `safe-coder chat --strategy orchestration` - Force parallel workers

**Internal behavior:**
- All modes: Proactive planning using L Core Integration
1. ✅ Rename `approval::ExecutionMode` to `ApLM
- Execution strategy auto-detected or explicit
- Parallel execution when beneficialprovalMode` (30 min)
2. ✅ Wire up `run
- Mode-aware planning (L_chat()` to use unified planning (1-2 hours)
3. ✅LM knows capabilities)

**Matrix of combinations:**

| Implement DirectExecutor with Session (2 hours)
4. ✅ Add basic CLI event Mode | Strategy | Behavior |
|------|----------|----------|
| plan | auto | Smart planning display (1 hour)

**Deliverable → show plan → ask approval → execute |
| plan**: `safe | direct | Sequential planning → show → approve → execute inline |
| plan | subagent-coder chat --mode build` works | Parallel planning → show → approve → spawn subagents |
| plan | orchestration | Parallel planning → show → approve → spawn workers | with unified planning

### Week 2: Full Execution
5. ✅ Implement Sub
| build | auto | Smart planning → auto-execute |
| build | direct | Sequential planningagentPlanExecutor integration (2 hours)
6. ✅ Implement OrchestrationExecutor integration (3 hours)
7. → auto-execute inline ✅ Add approval |
| build | subagent | Parallel planning → auto-execute with subagents |
| build | orchestration | Parallel planning → auto-execute with workers |

---

## File UI for Plan mode (2 hours)
8. ✅ Wire up `run_orchestrate()` (1 hour)

**Deliverable**: All three execution modes work Changes Checklist

### Core Types
- [x] `src/,unified_planning/types.rs` - Already done
- [ ] `src/approval approval flow complete

### Week 3: Polish
9. ✅ Add execution mode selection logic/mod.rs` - Rename (1 hour)
10. ✅ Add TUI mode toggle (2 hours)
11. ✅ Improve event ExecutionMode → ApprovalMode, Act → Build

### Executors
- [ display (1 hour)
12 ] `src/unified_planning/executors/direct.rs` - Wire. ✅ Write integration tests (2 hours)

**Deliverable**: Production-ready buil to session
- [ ] `src/unified_planning/executors/subagent.rs` - Wire to subagent executor
- [ ] `src/unified_planning/executors/orchestrationd/plan modes

---

## Key Design.rs` - Wire to worker/workspace

### Session Integration
- [ ] `src/session/mod.rs` - Update Decisions

### Decision 1: Keep Both Enums
** execution_mode field, add methodsRationale**: They serve different purposes (approval vs execution strategy)

### Decision 2: Auto for executors
- [ ] `src/main.rs` - Update run_chat() to use unified planning
- [ ] `src/commands-Select Execution Mode
**Rationale**: Users shouldn't need to understand Direct/mod.rs` - May need command updates

### Orchestrator Migration
- [ ] `src/orchest/Subagent/Orchestration - we pick the best one

### Decision 3: Executors Own Their Resourcesrator/mod.rs` - Use unified planner
- [ ] `src/orchestrator/planner.rs` - Deprec
**Rationale**: DirectExecutor needsate or remove
- [ ] `src/main.rs` - Update run_orchestrate()

### T Session, OrchestrationExecutor needs WorkspaceManager - pass them inUI Updates
- [ ] `src/tui/shell_app.rs` - Show planning UI, handle constructor

### Decision 4: Plan Mode = Unified Planning + events Approval
**Rationale**: Reuse the excellent
- [ ] `src/tui/mod.rs` - May need event type updates

### Tests
- [ ] Integration tests for each executor
- [ ] End-to-end tests for chat with planning system you built, just add approval gate

--- planning
- [ ] Tests for mode combinations

---

## Questions to

## Testing Strategy

### Unit Tests
- ✅ Each executor with mock dependencies
- ✅ Mode Resolve

### 1. Should orchestrator use the same planning system? selection heuristics
- ✅ Event emission

### Integration Tests
```rust
#[tokio::test]
async fn test_build_mode_direct_execution() {
    let result = run_chat_

**Current**: Orchestrator has its own `TaskPlan` anwith_input(
        PathBuf::from("test-project"),
        "build",d `PlanStep` types.

**Options**:
- A) // mode
        vec!["Add a test Migrate orchestrator to use `UnifiedPlan` ( file with hello world"],
    ).await.unwrap();
    
    assert!(result.success);
    assert!(cleaner, less code)
- B) Keep separate (orchestrator has specificPath::new("test-project/hello.txt").exists());
}

#[tokio::test]
async fn test_plan needs)

**Recommendation**: Option A - the unified planning system was designed for orchestration_mode_requires_approval() {
    let result = run_chat_with_input(
        PathBuf::from("test-project"),
        "plan", // mode
        vec!.

### 2. How should approval work with groups["Delete all files", "yes"], // approve
    ).await.unwrap();?

**Question**: If a plan has 3 groups
    
    assert!(result.required_approval);
    assert!(result.success);
}
```

---

## Migration Path for with 2 steps each, when do we ask approval?

**Options Existing Code

### Before
```rust
// Ol**:
- A) Show entire plan upfront, single approval
- B) Approved way - approval only
session.set_execution group by group
- C) Approve step by step

**Recommendation**: Option A_mode(ExecutionMode::Plan);
session.send_message("do something for Plan mode (matches").await?;
```

### After
```rust
// New way - approval + execution strategy
session.set_approval_mode(ApprovalMode::Plan);
let exec_mode = session.suggest_execution_mode("do something")?;

plan Claude Code), auto-approve all in Build mode.

### 3. Should strategy be auto-detected or explicit?

**Options**:
- A) Always_and_execute(
    "do something",
    exec_mode,
    session.llm_client(),
    project_path, auto-detect based on task
- B) Let user override with --
    config,
    None,
    session.requires_approval(),
).await?;
```

---

## Summary

Youstrategy flag
- C) Only auto-detect, no override

**Recommendation**: Option B -'ve built a sophisticated unified planning system but haven't connected it to the CLI. The good news: auto by default, allow override for power users.

### 4. What happens

✅ **Planning infrastructure is solid** - UnifiedPlanner, if Direct executor needs parallelism?

**Question**: User forces --strategy direct but task PlanRunner, executors all exist

⚠️ **Integration is missing has parallel steps** - Commands still use old approval system?

**Options**:
- A) Warn and execute sequentially anyway
- B) Error and refuse
- C) Override and use

🎯 **Clear path forward** - Rename for clarity, wire up execut subagent

**Recommendation**: Option A - respect user choice, warn about ineffors, add approval UI

**Total estimated time**: 2-3 daysiciency.

---

## Success Criteria

After full integration, for full integration

**Biggest wins**:
1. True users should be able to:

1. ✅ Run `safe-coder chat --mode plan` and see a Claude Code-style build/plan modes
2. Intelligent execution detailed execution plan before approval
2. ✅ Run `safe-coder chat --mode build` and have strategy selection (Direct/Subagent/Orchestration)
3. Better paral it auto-execute with planning
3. ✅ See parallel execution when appropriate (lelization and resource management
4. Cleaner separation of concerns

Letmultiple subagents or workers)
4. ✅ Override me know which phase you'd like to start with! execution strategy with `--strategy` flag
5. ✅ Get smart auto-detection of best strategy for each task
6. ✅ View real-time progress as plan executes (via events)
7. ✅ Have all three executors (direct, subagent, orchestration) fully functional

---

## Appendix: Code Examples

### Example: Using Unified Planning in Chat

```rust
async fn handle_chat_message(
    input: &str,
    approval_mode: ApprovalMode,
    strategy: ExecutionStrategy,
    session: &mut Session,
) -> Result<String> {
    // Get project context for planning
    let context = session.get_project_context().await?;
    
    // Create mode-aware planner
    let planner = UnifiedPlanner::new(strategy);
    
    // Generate plan
    let plan = planner
        .create_plan(session.llm_client(), input, Some(&context))
        .await?;
    
    // Build runner with appropriate approval callback
    let runner = PlanRunnerBuilder::new(session.project_path(), session.config())
        .with_registry(Arc::new(create_full_registry()))
        .require_approval(approval_mode.requires_approval())
        .build();
    
    // Add approval callback for Plan mode
    let runner = if approval_mode.requires_approval() {
        runner.with_approval_callback(|plan| {
            // Show plan to user in TUI or CLI
            println!("{}", plan.summary());
            
            // Ask for approval
            ask_user_approval("Execute this plan?")
        })
    } else {
        runner
    };
    
    // Execute plan
    let (completed_plan, mut events) = runner.execute(plan).await?;
    
    // Stream events to UI
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            // Forward to TUI or print to console
            handle_plan_event(event).await;
        }
    });
    
    // Return summary
    Ok(completed_plan.summary())
}
```

### Example: Executor Using Session

```rust
// src/unified_planning/executors/direct.rs
pub struct DirectExecutor {
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
}

impl DirectExecutor {
    async fn execute_step(&self, step: &UnifiedStep, ...) -> Result<StepResult> {
        // Build prompt from step
        let prompt = format!(
            "Execute the following task:\n\n{}\n\nInstructions: {}",
            step.description, step.instructions
        );
        
        // Send to LLM
        let response = self.llm_client
            .send_message(&[Message::user(prompt)], &self.tool_registry.all())
            .await?;
        
        // Execute any tool calls
        let mut tool_results = Vec::new();
        for tool_call in response.tool_calls {
            let result = self.tool_registry
                .execute(&tool_call.name, tool_call.parameters)
                .await?;
            tool_results.push(result);
        }
        
        // Build step result
        Ok(StepResult {
            success: true,
            output: format!("{}\n\nTools executed: {}", response.text, tool_results.len()),
            duration_ms: timer.elapsed_ms(),
            files_modified: extract_modified_files(&tool_results),
            error: None,
        })
    }
}
```

---

## Conclusion

The unified planning system is **architecturally sound and well-implemented**, but it's completely disconnected from the actual codebase. The integration work is straightforward but requires touching multiple files to wire everything together.

**Key insight**: The confusion stems from having two different concepts both called "ExecutionMode". Once we rename them (`ApprovalMode` vs `ExecutionStrategy`), the integration path becomes clear.

**Recommendation**: Start with Phase 1 (naming) and Phase 2 (direct executor) to get the basic flow working, then iterate from there. The unified planning system is already better than what Claude Code has - we just need to use it!