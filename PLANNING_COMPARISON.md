# Planning & Execution Comparison: Safe-Coder vs OpenCode

## Executive Summary

| Feature | Safe-Coder (Current) | OpenCode |
|---------|---------------------|----------|
| Planning Mode | Explicit Plan/Act modes | Agent-based (plan agent vs build agent) |
| Tool Approval | 4-mode (Plan/Default/AutoEdit/Yolo) | Permission matching + plugin hooks |
| System Prompts | Minimal (Claude Code compat only) | Hierarchical multi-layer |
| Context Management | Basic message history | Token-aware with auto-compaction |
| Streaming | Bash only | Full streaming with part-based messages |
| Tool Execution | Sequential only | Sequential with doom-loop detection |
| Error Recovery | Manual (git snapshots) | Retry with exponential backoff |

---

## How Safe-Coder Currently Works

### Planning Flow
```
User Input
    │
    ▼
┌─────────────────────────────────────┐
│  Send to LLM with all tools         │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  LLM responds with tool calls       │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Build ExecutionPlan from response  │
│  - Extract tool names               │
│  - Auto-assess risk levels          │
│  - Detect affected files            │
└─────────────────────────────────────┘
    │
    ├── Plan Mode ──► Show detailed plan → Ask approval
    │
    └── Act Mode ───► Show brief summary → Auto-execute
    │
    ▼
┌─────────────────────────────────────┐
│  Execute tools sequentially         │
│  Send results back to LLM           │
│  Loop until no more tool calls      │
└─────────────────────────────────────┘
```

### Strengths
- Clean separation (LLM, Tools, Approval, Session)
- Risk assessment per-tool (Low/Medium/High)
- Git auto-commit for safety
- Real-time progress events

### Weaknesses
1. **Reactive planning** - Plans generated AFTER LLM decides what to do
2. **No upfront exploration** - Doesn't analyze project before acting
3. **Minimal system prompts** - No guidance on HOW to plan
4. **No doom-loop detection** - Can repeat same failing action
5. **No context compression** - Will eventually hit token limits
6. **Memory manager disconnected** - Exists but not used in core loop

---

## How OpenCode Works

### Planning Flow
```
User Input
    │
    ▼
┌─────────────────────────────────────┐
│  Assemble Hierarchical System Prompt│
│  - Provider headers                 │
│  - Agent-specific prompts           │
│  - Custom/environment prompts       │
│  - Plugin transformations           │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Select Agent Mode                  │
│  - PLAN: Read-only tools only       │
│  - BUILD: Full execution capability │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Stream LLM Response                │
│  - Capture ReasoningPart (thinking) │
│  - Process ToolPart (execution)     │
│  - Handle TextPart (response)       │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Permission Gate per Tool Call      │
│  1. Check pre-approved patterns     │
│  2. Plugin hook evaluation          │
│  3. User confirmation if needed     │
│  4. Doom loop detection (3+ repeats)│
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│  Context Compression (if needed)    │
│  - Detect token overflow            │
│  - Prune completed tool outputs     │
│  - Generate session summary         │
└─────────────────────────────────────┘
```

### Key Innovations

1. **Agent-Based Tool Restriction**
   - PLAN agent: Only grep, find, git, cat, head, tail, ls, web fetch
   - BUILD agent: All tools including file edits and bash

2. **Hierarchical System Prompts**
   ```
   Provider-specific headers
        ↓
   Agent-specific prompts (PROMPT_PLAN, BUILD_SWITCH)
        ↓
   Custom/environment prompts
        ↓
   Message-level system prompts
        ↓
   Plugin hook transformation
   ```

3. **Permission Pattern Matching**
   - Users can approve "once" or "always" (pattern-based)
   - Patterns with wildcards for batch approvals

4. **Doom Loop Detection**
   - Detects when identical tool calls repeat 3+ times
   - Can auto-deny or ask user based on config

5. **Automatic Context Compaction**
   - Detects when approaching token limit
   - Generates summary of session progress
   - Prunes old tool outputs while preserving recent context

6. **Message Part System**
   - ReasoningPart: AI's internal thinking with timing
   - ToolPart: State machine (pending → running → completed/error)
   - StepStartPart/StepFinishPart: Boundaries with token metrics
   - SnapshotPart/PatchPart: File state tracking

---

## Enhancement Opportunities for Safe-Coder

### Priority 1: Agent-Based Planning Mode (High Impact)

**Current Problem**: Tools are always available; "Plan" mode just asks for approval but LLM still has access to all tools.

**OpenCode Solution**: Separate "plan" agent with restricted tools.

**Proposed Enhancement**:
```rust
pub enum AgentMode {
    Plan,   // Read-only: read, glob, grep, list, webfetch
    Build,  // Full: all tools including bash, edit, write
}

impl AgentMode {
    fn enabled_tools(&self) -> Vec<&str> {
        match self {
            AgentMode::Plan => vec![
                "read_file", "glob", "grep", "list_file", "webfetch"
            ],
            AgentMode::Build => vec![
                // All tools
            ],
        }
    }
}
```

### Priority 2: Hierarchical System Prompts (High Impact)

**Current Problem**: Only "You are Claude Code" - no guidance on planning, tool usage, or safety.

**Proposed Enhancement**:
```rust
fn build_system_prompt(&self, agent_mode: AgentMode) -> String {
    let mut prompt = String::new();
    
    // Base identity
    prompt.push_str(BASE_SYSTEM_PROMPT);
    
    // Agent-specific instructions
    match agent_mode {
        AgentMode::Plan => prompt.push_str(PLAN_AGENT_PROMPT),
        AgentMode::Build => prompt.push_str(BUILD_AGENT_PROMPT),
    }
    
    // Project context (from memory manager)
    if let Some(project_ctx) = self.memory.get_project_context() {
        prompt.push_str(&project_ctx);
    }
    
    // Tool guidance
    prompt.push_str(TOOL_USAGE_GUIDELINES);
    
    prompt
}

const PLAN_AGENT_PROMPT: &str = r#"
You are in PLAN mode. Your job is to explore and understand before acting.

Available tools: read_file, glob, grep, list_file, webfetch
NOT available: bash, write_file, edit_file (these require BUILD mode)

Guidelines:
1. Thoroughly explore the codebase before proposing changes
2. Identify all files that would need modification
3. Consider dependencies and side effects
4. Present a clear plan with specific file changes
5. Ask clarifying questions if requirements are unclear
"#;

const BUILD_AGENT_PROMPT: &str = r#"
You are in BUILD mode. Execute the planned changes carefully.

Guidelines:
1. Make changes incrementally, testing as you go
2. Prefer small, focused edits over large rewrites
3. Run tests after significant changes
4. If something fails, diagnose before retrying
5. Never retry the same failing action more than twice
"#;
```

### Priority 3: Doom Loop Detection (Medium Impact)

**Current Problem**: Can retry same failing action indefinitely.

**Proposed Enhancement**:
```rust
struct LoopDetector {
    recent_calls: VecDeque<(String, serde_json::Value)>,
    max_history: usize,
}

impl LoopDetector {
    fn check_doom_loop(&mut self, tool_name: &str, params: &serde_json::Value) -> Option<DoomLoopAction> {
        let call = (tool_name.to_string(), params.clone());
        
        // Count identical calls in recent history
        let count = self.recent_calls.iter()
            .filter(|c| c == &call)
            .count();
        
        self.recent_calls.push_back(call);
        if self.recent_calls.len() > self.max_history {
            self.recent_calls.pop_front();
        }
        
        if count >= 2 { // 3rd repeat
            Some(DoomLoopAction::AskUser {
                message: format!(
                    "Tool '{}' has been called 3 times with same parameters. Continue?",
                    tool_name
                ),
            })
        } else {
            None
        }
    }
}
```

### Priority 4: Context Compaction (Medium Impact)

**Current Problem**: No handling of token limits; conversations will eventually fail.

**Proposed Enhancement**:
```rust
impl Session {
    async fn maybe_compact_context(&mut self) -> Result<()> {
        let estimated_tokens = self.estimate_token_count();
        let max_context = self.llm_client.max_context_tokens();
        
        if estimated_tokens > max_context * 80 / 100 { // 80% threshold
            self.compact_context().await?;
        }
        
        Ok(())
    }
    
    async fn compact_context(&mut self) -> Result<()> {
        // 1. Generate summary of conversation so far
        let summary = self.generate_summary().await?;
        
        // 2. Prune old tool results (keep last N)
        self.prune_tool_results(5);
        
        // 3. Replace early messages with summary
        self.messages = vec![
            Message::system(format!("Session summary:\n{}", summary)),
            // Keep last N messages
            ..self.messages.drain(self.messages.len().saturating_sub(10)..)
        ];
        
        Ok(())
    }
}
```

### Priority 5: Permission Pattern Matching (Lower Impact)

**Current Problem**: Binary approve/deny per request; must approve similar actions repeatedly.

**Proposed Enhancement**:
```rust
struct PermissionManager {
    patterns: Vec<ApprovedPattern>,
}

struct ApprovedPattern {
    tool_name: String,
    param_patterns: HashMap<String, String>, // key -> glob pattern
}

impl PermissionManager {
    fn check(&self, tool_name: &str, params: &serde_json::Value) -> Permission {
        for pattern in &self.patterns {
            if pattern.matches(tool_name, params) {
                return Permission::Allowed;
            }
        }
        Permission::NeedsApproval
    }
    
    fn approve_pattern(&mut self, tool_name: &str, param_patterns: HashMap<String, String>) {
        // User says "always approve edits to src/**/*.rs"
        self.patterns.push(ApprovedPattern {
            tool_name: tool_name.to_string(),
            param_patterns,
        });
    }
}
```

---

## Implementation Roadmap

### Phase 1: Foundation (1-2 days)
1. Add `AgentMode` enum with tool filtering
2. Integrate MemoryManager into session's system prompt
3. Add hierarchical system prompt builder

### Phase 2: Safety (1-2 days)
4. Implement doom loop detection
5. Add retry with exponential backoff for transient errors
6. Better error categorization (retryable vs fatal)

### Phase 3: Scalability (2-3 days)
7. Implement token estimation
8. Add context compaction with summary generation
9. Prune tool outputs while preserving recent context

### Phase 4: UX Polish (1-2 days)
10. Permission pattern matching
11. "Approve always" option in TUI
12. Better display of AI reasoning/thinking

---

## File Changes Required

| File | Changes |
|------|---------|
| `src/session/mod.rs` | Add AgentMode, doom loop, context compaction |
| `src/llm/mod.rs` | Add system prompt builder, token estimation |
| `src/tools/mod.rs` | Add tool filtering by AgentMode |
| `src/approval/mod.rs` | Add permission patterns |
| `src/memory/mod.rs` | Integrate with session (it's disconnected now) |
| `src/tui/shell_app.rs` | Add AgentMode toggle, permission UI |

