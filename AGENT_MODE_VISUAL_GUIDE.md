# Agent Mode Sidebar - Visual Guide

## Before & After Comparison

### BEFORE: No Agent Mode Display in Sidebar

```
┌─────────────────────────────────────────────────────────────┐
│ Safe Coder Shell                                            │
├─────────────────────────────────────────────────────────────┤
│                                          ┌──────────────────┐│
│  $ ls -la                                │  TASK            ││
│  drwxr-xr-x  10 user  staff   320 ...   │  Analyzing code  ││
│                                          │                  ││
│  $ /connect                              ├──────────────────┤│
│  Connected to AI                         │  CONTEXT         ││
│                                          │  42.1k tokens    ││
│  > Fix the authentication bug            ├──────────────────┤│
│                                          │  FILES           ││
│  AI: Analyzing the code...               │  + auth.rs       ││
│  [Tool: read_file auth.rs]               │  ~ main.rs       ││
│  [Tool: edit_file auth.rs]               ├──────────────────┤│
│                                          │  PLAN            ││
│  $ cargo test                            │  Step 1: ✓       ││
│  Running tests...                        │  Step 2: ⋯       ││
│  test auth::test ... ok                  │  Step 3: ○       ││
│                                          ├──────────────────┤│
│  > _                                     │  LSP             ││
│                                          │  ✓ rust-analyzer ││
│                                          └──────────────────┘│
└─────────────────────────────────────────────────────────────┘
Status: ~/project                                        BUILD
```

**Issues:**
- ❌ Agent mode only visible in bottom status bar
- ❌ Easy to miss current mode
- ❌ No indication of tool restrictions
- ❌ Hard to understand PLAN vs BUILD difference

---

### AFTER: Agent Mode Display in Sidebar ✨

```
┌─────────────────────────────────────────────────────────────┐
│ Safe Coder Shell                                            │
├─────────────────────────────────────────────────────────────┤
│                                          ┌──────────────────┐│
│  $ ls -la                                │  TASK            ││
│  drwxr-xr-x  10 user  staff   320 ...   │  Analyzing code  ││
│                                          │                  ││
│  $ /connect                              ├──────────────────┤│
│  Connected to AI                         │  MODE         ★  ││
│                                          │  BUILD - Full... ││
│  > Fix the authentication bug            ├──────────────────┤│
│                                          │  CONTEXT         ││
│  AI: Analyzing the code...               │  42.1k tokens    ││
│  [Tool: read_file auth.rs]               ├──────────────────┤│
│  [Tool: edit_file auth.rs]               │  FILES           ││
│                                          │  + auth.rs       ││
│  $ cargo test                            │  ~ main.rs       ││
│  Running tests...                        ├──────────────────┤│
│  test auth::test ... ok                  │  PLAN            ││
│                                          │  Step 1: ✓       ││
│  > _                                     │  Step 2: ⋯       ││
│                                          │  Step 3: ○       ││
│                                          ├──────────────────┤│
│                                          │  LSP             ││
│                                          │  ✓ rust-analyzer ││
│                                          └──────────────────┘│
└─────────────────────────────────────────────────────────────┘
Status: ~/project                                        BUILD
```

**Improvements:**
- ✅ Prominent MODE section in sidebar
- ✅ Color-coded indicator (GREEN/CYAN)
- ✅ Brief mode description
- ✅ Consistent with status bar

---

## Visual Mode Indicators

### BUILD Mode (Full Execution)

```
┌──────────────────┐
│  MODE            │
│  BUILD - Full... │  ← GREEN (RGB 120, 200, 120)
└──────────────────┘
```

**Visual Cues:**
- 🟢 **Bright Green** color
- **BOLD** text style
- Description: "Full execution mode"

**Meaning:**
- All tools available
- Can modify files
- Can run shell commands
- Full AI capabilities

---

### PLAN Mode (Read-Only)

```
┌──────────────────┐
│  MODE            │
│  PLAN - Read-... │  ← CYAN (RGB 80, 200, 220)
└──────────────────┘
```

**Visual Cues:**
- 🔵 **Cyan Blue** color
- **BOLD** text style
- Description: "Read-only exploration mode"

**Meaning:**
- Limited to read tools
- Cannot modify files
- Cannot run commands
- Safe exploration only

---

## Mode Switching Animations

### Using Ctrl+G

```
Before:                    After:
┌────────────────┐        ┌────────────────┐
│  MODE          │  Ctrl+G │  MODE          │
│  BUILD - F...  │  ────→  │  PLAN - R...   │
└────────────────┘        └────────────────┘
    🟢 GREEN                   🔵 CYAN
```

### Using /agent Command

```
User Input:
> /agent

System Response:
┌─────────────────────────────────────┐
│ Agent mode: PLAN - Read-only...     │
└─────────────────────────────────────┘

Sidebar Updates:
┌────────────────┐
│  MODE          │
│  PLAN - R...   │  ← Changes color & text
└────────────────┘
```

---

## Complete Sidebar Layout

### Full View with All Sections

```
┌────────────────────────────────┐
│  TASK                          │  ← Current task description
│  Fixing authentication bug     │
│  Analyzing auth flow...        │
├────────────────────────────────┤
│  MODE                    ★ NEW │  ← Agent mode indicator
│  BUILD - Full execution mode   │     (GREEN for BUILD)
├────────────────────────────────┤
│  CONTEXT                       │  ← Token usage
│  42.1k / 200k (21%)           │
│  Cache: 15k tokens saved       │
├────────────────────────────────┤
│  FILES                         │  ← Modified files
│  + auth.rs                     │
│  ~ main.rs                     │
│  - old_auth.rs                 │
├────────────────────────────────┤
│  PLAN                          │  ← Current plan steps
│  ✓ Step 1: Read auth.rs        │
│  ⋯ Step 2: Analyze flow         │
│  ○ Step 3: Fix bug              │
│  ○ Step 4: Add tests            │
├────────────────────────────────┤
│  LSP                           │  ← Language servers
│  ✓ rust-analyzer               │
│  ✓ typescript-language-server  │
└────────────────────────────────┘
```

### Section Heights

```
TASK:     4 lines (fixed)
MODE:     3 lines (fixed) ★ NEW
CONTEXT:  3 lines (fixed)
FILES:    2-7 lines (dynamic)
PLAN:     6+ lines (flexible)
LSP:      5 lines (fixed)
```

---

## Color Scheme Reference

### MODE Section Colors

| Mode  | Color Name | RGB Values      | Hex Code | Usage                    |
|-------|-----------|-----------------|----------|--------------------------|
| BUILD | Green     | (120, 200, 120) | #78C878  | Full execution mode      |
| PLAN  | Cyan      | (80, 200, 220)  | #50C8DC  | Read-only mode           |

### Text Hierarchy

| Element     | Color        | RGB Values      | Style      |
|-------------|--------------|-----------------|------------|
| Header      | Dim          | (100, 100, 110) | BOLD       |
| Mode Name   | Green/Cyan   | Varies          | BOLD       |
| Description | Secondary    | (150, 150, 160) | Regular    |

---

## Interactive Examples

### Example 1: Starting in PLAN Mode

```
Step 1: Launch Safe Coder
$ safe-coder /path/to/project

Step 2: Connect AI
> /connect
Connected to AI

Step 3: Check Mode (Sidebar shows)
┌────────────────┐
│  MODE          │
│  BUILD - F...  │  ← Default mode
└────────────────┘

Step 4: Switch to PLAN
> /agent
Agent mode: PLAN - Read-only exploration mode

Step 5: Sidebar Updates
┌────────────────┐
│  MODE          │
│  PLAN - R...   │  ← Now in PLAN mode (CYAN)
└────────────────┘

Step 6: Try to modify file
> Edit the auth.rs file

AI Response:
⚠️  Tool 'edit_file' not available in PLAN mode
💡  Switch to BUILD mode with /agent or Ctrl+G
```

### Example 2: Safe Exploration

```
Workflow: Explore before changing

1. Start in PLAN mode
   > /agent  (if in BUILD)
   Mode: PLAN - Read-only

2. Explore codebase safely
   > Read all files in src/
   > Search for authentication logic
   > Analyze the code structure

3. Sidebar shows:
   ┌────────────────┐
   │  MODE          │
   │  PLAN - R...   │  ← Safe mode
   └────────────────┘
   
   ┌────────────────┐
   │  FILES         │
   │  No changes    │  ← Cannot modify
   └────────────────┘

4. Ready to make changes
   > /agent
   Mode: BUILD - Full execution

5. Implement fixes
   > Fix the bug in auth.rs
   [AI can now use edit_file, write_file, bash]

6. Sidebar updates:
   ┌────────────────┐
   │  MODE          │
   │  BUILD - F...  │  ← Can modify now
   └────────────────┘
   
   ┌────────────────┐
   │  FILES         │
   │  ~ auth.rs     │  ← Files being changed
   └────────────────┘
```

---

## Troubleshooting

### Mode Not Changing?

**Problem:** MODE section shows wrong mode after switching

**Solution:**
```
1. Check status bar (bottom right) - should match sidebar
2. Try switching again with Ctrl+G
3. Verify with: /agent
4. Restart if issue persists
```

### Color Not Showing?

**Problem:** MODE section is white/gray instead of GREEN/CYAN

**Solution:**
```
1. Ensure terminal supports 24-bit color
2. Check $TERM variable (should be xterm-256color or similar)
3. Try: export TERM=xterm-256color
4. Restart Safe Coder
```

### MODE Section Missing?

**Problem:** Sidebar doesn't show MODE section

**Solution:**
```
1. Check if sidebar is visible (toggle with Ctrl+S)
2. Ensure terminal height is sufficient (>= 30 lines)
3. Update to latest version
4. Check build was successful
```

---

## Keyboard Shortcuts Summary

```
┌──────────────────────────────────────────────────────┐
│  Agent Mode Controls                                 │
├──────────────────────────────────────────────────────┤
│  Ctrl+G           Cycle agent mode (PLAN ↔ BUILD)    │
│  /agent           Toggle agent mode via command      │
│  /help            Show all commands                  │
├──────────────────────────────────────────────────────┤
│  Permission Mode Controls                            │
├──────────────────────────────────────────────────────┤
│  Ctrl+P           Cycle permission mode              │
│  /mode            Toggle permission mode             │
└──────────────────────────────────────────────────────┘
```

---

## Tool Availability by Mode

### PLAN Mode (7 tools)

```
┌────────────────────────────────────────┐
│  READ ONLY TOOLS                       │
├────────────────────────────────────────┤
│  ✓ read_file      Read file contents   │
│  ✓ list_file      List directories     │
│  ✓ glob           Find files by pattern│
│  ✓ grep           Search file contents │
│  ✓ ast_grep       AST code search      │
│  ✓ webfetch       Fetch web content    │
│  ✓ todoread       Read task list       │
└────────────────────────────────────────┘
```

### BUILD Mode (13 tools)

```
┌────────────────────────────────────────┐
│  ALL TOOLS (includes PLAN + below)     │
├────────────────────────────────────────┤
│  ✓ write_file     Create/overwrite     │
│  ✓ edit_file      Modify files         │
│  ✓ bash           Run shell commands   │
│  ✓ todowrite      Update task list     │
│  ✓ build_config   Build configuration  │
│  + all PLAN tools                      │
└────────────────────────────────────────┘
```

---

## Best Practices

### When to Use PLAN Mode

```
✅ Initial code exploration
✅ Understanding architecture
✅ Searching for patterns
✅ Analyzing before changes
✅ Code review sessions
✅ Learning new codebase
```

### When to Use BUILD Mode

```
✅ Implementing features
✅ Fixing bugs
✅ Refactoring code
✅ Running tests
✅ Generating files
✅ Executing commands
```

### Recommended Workflow

```
1. Start: PLAN mode
   ├─ Explore codebase
   ├─ Understand structure
   └─ Identify changes needed

2. Switch: BUILD mode
   ├─ Implement changes
   ├─ Run tests
   └─ Verify results

3. Return: PLAN mode
   ├─ Review changes
   └─ Plan next steps
```

---

## Implementation Details

### Files Modified

```
src/tui/shell_ui.rs       (+42 lines)
├─ draw_sidebar_mode()    New function
└─ draw_sidebar()         Updated layout

src/tui/shell_app.rs      (+3 lines)
├─ SlashCommand::Agent    New variant
└─ parse_slash_command()  Updated parser

src/tui/shell_runner.rs   (+25 lines)
├─ handle Agent command   New handler
└─ Help text              Updated docs
```

### Code Metrics

```
Total Lines Added:    70
Files Modified:       3
Breaking Changes:     0
Test Coverage:        N/A (TUI component)
Build Time Impact:    < 1 second
Runtime Performance:  Negligible
```

---

## Future Vision

### Planned Enhancements

```
┌────────────────────────────────────┐
│  MODE                 (Ctrl+G)     │  ← Keyboard hint
│  BUILD - Full execution            │
│  📊 13 tools available              │  ← Tool count
│                                    │
│  ▼ View restrictions               │  ← Expandable
└────────────────────────────────────┘

Expanded:
┌────────────────────────────────────┐
│  MODE                 (Ctrl+G)     │
│  BUILD - Full execution            │
│  📊 13 tools available              │
│  ▼ Hide restrictions               │
│                                    │
│  ✓ All PLAN tools                  │
│  ✓ write_file, edit_file          │
│  ✓ bash (shell execution)          │
│  ✓ todowrite, build_config         │
└────────────────────────────────────┘
```

---

## Conclusion

The agent mode sidebar feature provides clear, real-time visibility into the current execution mode, improving both safety and user experience. The implementation follows Safe Coder's design principles of clarity, consistency, and user-friendly interaction.

**Key Takeaways:**
- 🎯 Prominent visual indicator
- 🎨 Color-coded for quick recognition
- ⌨️ Multiple ways to switch (Ctrl+G, /agent)
- 📊 Consistent with existing UI
- 🔒 Enhanced safety awareness