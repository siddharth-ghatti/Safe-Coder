# Oh-My-OpenCode vs Safe-Coder: Gap Analysis & Implementation Plan

## Executive Summary

**Oh-My-OpenCode** is a plugin for OpenCode that adds sophisticated agent orchestration, lifecycle hooks, and IDE-grade tools. After thorough analysis, Safe-Coder already has **many of these features** implemented natively in Rust, but there are **key gaps** that would make Safe-Coder more competitive.

---

## Feature Comparison Matrix

| Feature | Oh-My-OpenCode | Safe-Coder | Gap? |
|---------|----------------|------------|------|
| **Multi-Model Agents** | 7 agents (Claude, GPT, Gemini, Grok) | 5 subagents (same LLM) | **GAP** |
| **LSP Integration** | 11 tools (refactor, rename, actions) | ✅ Full (auto-download) | **Safe-Coder Better** |
| **AST-Grep** | ✅ Pattern matching | ❌ Missing | **GAP** |
| **Context Window Monitor** | ✅ 85% threshold | ✅ Auto-compaction | Equivalent |
| **Todo Continuation Enforcer** | ✅ Prevents abandonment | ✅ Todo tracking | Equivalent |
| **Background Tasks** | ✅ Parallel agents | ✅ Orchestrator | Equivalent |
| **Hooks System** | 22 lifecycle hooks | ❌ Missing | **GAP** |
| **AGENTS.md Injection** | ✅ Auto-inject from dirs | ✅ SAFE_CODER.md | Equivalent |
| **Comment Checker** | ✅ Prevents spam | ❌ Missing | **Minor GAP** |
| **Session Recovery** | ✅ Auto-resume | ✅ Persistence | Equivalent |
| **Tool Output Truncation** | ✅ Configurable | ✅ Built-in | Equivalent |
| **Skill System** | ✅ MCP-based skills | ❌ Missing | **GAP** |
| **Rules Injector** | ✅ Pattern-based rules | ❌ Missing | **GAP** |
| **Interactive Bash** | ✅ Session-based | ✅ Shell TUI | **Safe-Coder Better** |
| **Claude Code Compat** | ✅ Hooks/commands/skills | ❌ Own format | Different approach |
| **Preemptive Compaction** | ✅ At 85% | ✅ `/compact` command | Equivalent |
| **Keyword Detection** | ✅ "ultrawork", "analyze" | ❌ Missing | **Minor GAP** |
| **Think Mode** | ✅ Extended thinking | ❌ Missing | **GAP** |
| **Multi-Model Support** | ✅ Per-agent models | ✅ OpenRouter (75+) | **Safe-Coder Better** |
| **Checkpoints** | ❌ Git only | ✅ Git-agnostic | **Safe-Coder Better** |
| **Undo/Redo** | ❌ | ✅ `/undo` `/redo` | **Safe-Coder Better** |

---

## Gap Analysis: What Oh-My-OpenCode Has That We Don't

### 1. **Multi-Model Agent Orchestration** (HIGH PRIORITY)
Oh-My-OpenCode assigns *different LLM providers* to different agent types:
- **Sisyphus** (Claude Opus 4.5) - Main orchestrator
- **Oracle** (GPT-5.2) - Strategic reasoning
- **Librarian** (Claude Sonnet) - Documentation
- **Explore** (Grok) - Fast search
- **Frontend Engineer** (Gemini) - UI generation

**Safe-Coder Gap**: Our subagents all use the same LLM. We should allow per-subagent model configuration.

### 2. **AST-Grep Tool** (MEDIUM PRIORITY)
Structural code search using AST patterns, not just text regex. Supports 25+ languages.

**Safe-Coder Gap**: We only have text-based grep. AST-aware search would be much more powerful.

### 3. **Hooks System** (MEDIUM PRIORITY)
22 lifecycle hooks including:
- `PreToolUse` / `PostToolUse` - Before/after tool execution
- `UserPromptSubmit` - Before processing user input
- `Stop` - Session end cleanup
- `context-window-monitor` - Track token usage
- `todo-continuation-enforcer` - Ensure task completion
- `comment-checker` - Prevent excessive comments
- `edit-error-recovery` - Auto-fix edit failures

**Safe-Coder Gap**: We have no hook system. Users can't customize behavior.

### 4. **Skill System** (MEDIUM PRIORITY)
Loadable skill files (`.md` or MCP servers) that inject specialized knowledge:
- Language-specific patterns
- Framework conventions
- Project-specific rules

**Safe-Coder Gap**: We have `SAFE_CODER.md` but no dynamic skill loading.

### 5. **Rules Injector** (LOW PRIORITY)
Pattern-based rule injection. When a file matching `*.tsx` is edited, inject React rules.

**Safe-Coder Gap**: Missing but could be added to LSP integration.

### 6. **Think Mode / Extended Thinking** (LOW PRIORITY)
Detect when complex reasoning is needed and enable extended thinking.

**Safe-Coder Gap**: We show inline reasoning but don't have a formal "think mode".

### 7. **Keyword Detection** (LOW PRIORITY)
Magic keywords like "ultrawork" that activate special modes.

**Safe-Coder Gap**: Not really needed - we have explicit commands.

---

## What Safe-Coder Does Better

1. **Native Binary (Rust)** - 20x faster startup vs Node.js
2. **OpenRouter Integration** - 75+ models with one API key
3. **Git-Agnostic Checkpoints** - Works without git
4. **Undo/Redo** - Instant rollback with `/undo` `/redo`
5. **LSP Auto-Download** - Automatically installs language servers
6. **Multi-CLI Orchestration** - Delegates to Claude Code, Gemini CLI, Copilot
7. **Warp-like Shell TUI** - Modern terminal interface

---

## Implementation Plan

### Sprint 1: Multi-Model Subagents (TODAY - 2-3 hours)

**Goal**: Allow each subagent type to use a different LLM provider/model.

**Files to modify**:
- `src/config.rs` - Add subagent model config
- `src/subagent/types.rs` - Add model field to SubagentType
- `src/subagent/executor.rs` - Create LLM client per subagent
- `src/subagent/mod.rs` - Wire up model selection

**Config example**:
```toml
[subagents]
analyzer.model = "anthropic/claude-3.5-sonnet"
tester.model = "openai/gpt-4o"
refactorer.model = "anthropic/claude-3-opus"
documenter.model = "google/gemini-pro-1.5"
```

### Sprint 2: AST-Grep Tool (TODAY - 1-2 hours)

**Goal**: Add AST-aware code search using tree-sitter.

**Files to create/modify**:
- `src/tools/ast_grep.rs` (new) - AST pattern matching
- `src/tools/mod.rs` - Register tool
- `Cargo.toml` - Add tree-sitter crates

**Implementation**:
- Use `tree-sitter` crate for parsing
- Support patterns like `fn $NAME($ARGS) { $BODY }`
- Cover Rust, TypeScript, Python, Go initially

### Sprint 3: Hooks System (TODAY - 2-3 hours)

**Goal**: Add customizable lifecycle hooks.

**Files to create/modify**:
- `src/hooks/mod.rs` (new) - Hook system
- `src/hooks/types.rs` (new) - Hook types
- `src/hooks/builtin.rs` (new) - Built-in hooks
- `src/session/mod.rs` - Call hooks at lifecycle points
- `src/tools/mod.rs` - Call pre/post tool hooks

**Hook types**:
```rust
enum HookType {
    PreToolUse,      // Before any tool executes
    PostToolUse,     // After tool completes
    PrePrompt,       // Before sending to LLM
    PostResponse,    // After LLM response
    SessionStart,    // Session begins
    SessionEnd,      // Session ends
}
```

**Built-in hooks**:
- `CommentChecker` - Warn if too many comments added
- `ContextMonitor` - Track token usage, warn at 85%
- `TodoEnforcer` - Ensure todos are completed

### Sprint 4: Skill System (OPTIONAL - 1-2 hours)

**Goal**: Load skill files for specialized knowledge.

**Files to create/modify**:
- `src/skills/mod.rs` (new) - Skill loader
- `src/skills/types.rs` (new) - Skill types
- `.safe-coder/skills/` - Skill directory

**Skill format**:
```markdown
---
name: react-patterns
trigger: "*.tsx"
---

# React Best Practices

When working with React components:
1. Use functional components with hooks
2. Prefer composition over inheritance
...
```

---

## Priority Order

| Priority | Feature | Time | Impact |
|----------|---------|------|--------|
| 1 | Multi-Model Subagents | 2-3h | HIGH - Key differentiator |
| 2 | AST-Grep Tool | 1-2h | HIGH - Better code search |
| 3 | Hooks System | 2-3h | MEDIUM - Extensibility |
| 4 | Skill System | 1-2h | LOW - Nice to have |

**Total estimated time: 6-10 hours**

---

## Recommendation

Start with **Multi-Model Subagents** as it's the highest-impact feature that directly competes with oh-my-opencode's main selling point. Then add **AST-Grep** for better code intelligence. The hooks and skill system can come later as polish.

After implementation, Safe-Coder will have:
- ✅ Multi-model agent orchestration (like oh-my-opencode)
- ✅ AST-aware code search (like oh-my-opencode)
- ✅ Plus all our unique features (Rust speed, checkpoints, undo/redo, shell TUI)
