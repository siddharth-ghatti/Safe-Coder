# Unified Planning Integration Checklist

## Overview

Integration of unified planning system into Safe-Coder shell for build and plan modes.

---

## Phase 1: Rename and Clarify ✅ COMPLETE

### Goal
Eliminate confusion between approval workflow and execution strategy by renaming types.

### Tasks
- [x] Rename `approval::ExecutionMode` → `approval::UserMode`
- [x] Rename `Act` variant → `Build`
- [x] Update `UserMode::from_str()` to accept "build" (and "act" for backward compat)
- [x] Update `UserMode::as_str()` to return "build"
- [x] Update default from `Act` to `Build`
- [x] Update all references in `session/mod.rs`
  - [x] Import `UserMode` instead of `ExecutionMode`
  - [x] Field `execution_mode` → `user_mode`
  - [x] Method `set_execution_mode()` → `set_user_mode()`
  - [x] Method `execution_mode()` → `user_mode()`
  - [x] Update match arms for `UserMode::Plan` and `UserMode::Build`
- [x] Update all references in `main.rs`
  - [x] `run_chat()` - use `UserMode`
  - [x] `run_orchestrate()` - use `UserMode`
  - [x] Update mode descriptions ("BUILD" instead of "ACT")
- [x] Update all references in `orchestrator/mod.rs`
  - [x] Import `UserMode` instead of `ExecutionMode`
  - [x] Field `execution_mode` → `user_mode`
  - [x] Update match arms and tests
- [x] Update all references in `commands/slash.rs`
  - [x] SlashCommand::ExecutionMode handler
  - [x] Update descriptions
- [x] Update tests in `approval/mod.rs`
- [x] Verify compilation with `cargo check`

### Verification
```bash
cargo check
# Should compile without ExecutionMode errors
```

---

## Phase 2: Wire Up DirectExecutor ✅ COMPLETE

### Goal
Make DirectExecutor actually execute steps using LLM and tools instead of returning placeholders.

### Tasks
- [x] Expand `ExecutorContext` to include execution dependencies
  - [x] Add `llm_client: Arc<dyn LlmClient>` field
  - [x] Add `tool_registry: Arc<ToolRegistry>` field
  - [x] Update constructor to accept these parameters
  - [x] Update tests to provide these parameters
- [x] Update `PlanRunner` to pass execution dependencies
  - [x] Add `llm_client` and `tool_registry` fields
  - [x] Update constructor signature
  - [x] Pass to `ExecutorContext::new()` when creating context
  - [x] Update all tests
- [x] Update `PlanRunnerBuilder` for execution dependencies
  - [x] Add `llm_client` and `tool_registry` fields
  - [x] Add `with_llm_client()` method
  - [x] Add `with_tool_registry()` method
  - [x] Update `build()` to require these (expect/unwrap)
  - [x] Update tests
- [x] Update integration helpers
  - [x] `create_runner()` - accept and pass llm_client and tool_registry
  - [x] `create_runner_with_approval()` - same
  - [x] `plan_and_execute()` - accept Arc<dyn LlmClient> and Arc<ToolRegistry>
- [x] Implement real execution in `DirectExecutor`
  - [x] Remove TODO comments and placeholder code
  - [x] Build context message from step instructions
  - [x] Get tool definitions from registry (`get_tools_schema()`)
  - [x] Convert to `ToolDefinition` format for LLM
  - [x] Call `llm_client.send_message()` with tools
  - [x] Extract text and `ContentBlock::ToolUse` from response
  - [x] Create `ToolContext` with proper fields
  - [x] Execute each tool via `tool.execute()`
  - [x] Track modified files
  - [x] Handle errors gracefully
  - [x] Build `StepResult` with actual output/errors
  - [x] Emit progress events throughout
- [x] Update tests
  - [x] Provide LLM client and tool registry in test contexts
  - [x] Handle cases where LLM may not be configured

### Verification
```bash
cargo build --quiet
# Should compile without errors
```

---

## Phase 3: Integrate Unified Planning into Shell 🚧 IN PROGRESS

### Goal
Replace shell's reactive tool loop with proactive unified planning system for both plan and build modes.

### Current State
- Shell uses `AgentMode` (Plan/Build) for tool filtering
- Shell has `PermissionMode` (Ask/Edit/Yolo) for approval granularity
- No unified planning integration yet

### Tasks

#### 3.1: Add Unified Planning to Session
- [ ] Add unified planning fields to `Session`
  - [ ] Add `use crate::unified_planning::*;`
  - [ ] Add optional unified planning mode flag
  - [ ] Keep existing send_message for backward compat
- [ ] Add new method `Session::send_message_with_planning()`
  - [ ] Check if agent_mode is Plan or Build
  - [ ] Map to `UnifiedExecutionMode::Direct` (inline execution)
  - [ ] Create `UnifiedPlanner` with execution mode
  - [ ] Call `planner.create_plan()`
  - [ ] Create `PlanRunner` with approval based on user_mode
  - [ ] Execute plan and stream events
  - [ ] Return summary
- [ ] Add method `Session::get_llm_client()` → `Arc<dyn LlmClient>`
- [ ] Add method `Session::get_tool_registry()` → `Arc<ToolRegistry>`
- [ ] Emit `SessionEvent::Plan(PlanEvent)` for plan events

#### 3.2: Update Shell TUI Integration
- [ ] Update `ShellTuiApp` to display plan execution
  - [ ] Add field for current plan (if any)
  - [ ] Add field for plan events stream
  - [ ] Add method to handle `SessionEvent::Plan(event)`
- [ ] Add plan visualization in output
  - [ ] Show plan steps when created
  - [ ] Show step progress indicators
  - [ ] Show completed/failed steps with icons
  - [ ] Show group parallelism (if any in future)
- [ ] Add approval UI for plan mode
  - [ ] Detect when plan is awaiting approval
  - [ ] Show approval prompt in status bar
  - [ ] Handle 'y' key to approve, 'n' to reject
  - [ ] Emit approval via event channel

#### 3.3: Wire Shell to Use Unified Planning
- [ ] Update `ShellTuiApp::handle_ai_input()` or equivalent
  - [ ] Check if session has planning enabled
  - [ ] Call `session.send_message_with_planning()` instead of `send_message()`
  - [ ] Handle plan events in real-time
  - [ ] Display plan before execution (if plan mode)
  - [ ] Auto-execute or wait for approval
- [ ] Update shell command line mode (non-TUI)
  - [ ] Same changes but for CLI output
  - [ ] Pretty-print plan to console
  - [ ] Read approval from stdin

#### 3.4: Mode Integration
- [ ] Map AgentMode to execution strategy
  - [ ] `AgentMode::Plan` → Explore-first, show plan, wait for approval
  - [ ] `AgentMode::Build` → Quick planning, auto-execute
- [ ] Keep PermissionMode for per-tool approval (orthogonal to unified planning)
- [ ] Document mode interactions in code

#### 3.5: Testing
- [ ] Manual test: shell with plan mode
  - [ ] `cargo run -- shell --ai`
  - [ ] Toggle to plan mode (Ctrl+M)
  - [ ] Send AI request
  - [ ] Verify plan is shown
  - [ ] Approve and verify execution
- [ ] Manual test: shell with build mode
  - [ ] Same but in build mode
  - [ ] Verify auto-execution without approval
- [ ] Test tool execution works correctly
- [ ] Test error handling
- [ ] Test progress display

### Verification
```bash
# Test plan mode
cargo run -- shell --ai
# In shell: Ctrl+M to toggle to Plan mode
# Send request, verify plan shown and approval required

# Test build mode  
# In shell: Ctrl+M to toggle to Build mode
# Send request, verify auto-execution
```

---

## Phase 4: Polish and Documentation (Future)

### Tasks
- [ ] Add keyboard shortcut to toggle plan/build mode (Ctrl+M already exists)
- [ ] Add mode indicator in status bar
- [ ] Improve plan visualization (colors, formatting)
- [ ] Add ability to edit plan before approval
- [ ] Document unified planning in README
- [ ] Add examples of plan vs build mode
- [ ] Update CLI help text
- [ ] Add configuration options for execution strategy
- [ ] Performance testing and optimization

---

## Success Criteria

- [x] Phase 1: Code compiles with UserMode instead of ExecutionMode
- [x] Phase 2: DirectExecutor executes real tools via LLM
- [ ] Phase 3: Shell uses unified planning for AI requests
- [ ] Shell shows plan before execution in plan mode
- [ ] Shell auto-executes in build mode
- [ ] Plan events are visualized in TUI
- [ ] Approval workflow works correctly
- [ ] Tool execution produces actual results
- [ ] Error handling is robust

---

## Notes

### Design Decisions

1. **UserMode vs ExecutionMode**: `UserMode` (Plan/Build) controls approval workflow. `ExecutionMode` (Direct/Subagent/Orchestration) controls execution strategy. They are orthogonal.

2. **Direct Execution for Shell**: Shell always uses `ExecutionMode::Direct` since it's inline execution. Future: could auto-detect complexity and suggest Subagent mode.

3. **AgentMode Integration**: AgentMode (Plan/Build) in tools module is kept for tool filtering. It works alongside UserMode for approval.

4. **Backward Compatibility**: Old "act" mode maps to "build" mode for backward compatibility.

### Future Enhancements

- Auto-detect execution strategy based on request complexity
- Support Subagent execution mode from shell
- Add plan editing capability
- Add plan history/replay
- Add plan templates
- Integration with orchestration mode for large tasks

---

## Timeline

- Phase 1: ✅ Complete (30 min)
- Phase 2: ✅ Complete (2 hours)
- Phase 3: 🚧 In Progress (estimated 2-3 hours)
- Phase 4: Future

**Total estimated**: ~5-6 hours for phases 1-3