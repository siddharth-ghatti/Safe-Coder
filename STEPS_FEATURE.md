# Steps Feature - Build Mode Execution Tracking

## Overview

The **Steps** feature provides real-time tracking and visualization of all tool executions when SafeCoder is in **Build Mode**. This gives you a live, scrollable list of every action the AI agent is taking to complete your task.

## What is Build Mode?

Build mode is one of two agent modes in SafeCoder:

- **Plan Mode**: Shows a todo-style checklist of planned tasks
- **Build Mode**: Shows real-time step-by-step tool executions

Toggle between modes with `Ctrl+G`.

## Features

### Real-Time Step Tracking

When in Build Mode, the sidebar's "PLAN" section becomes "STEPS" and displays:

- ✓ **Completed steps** - Successfully executed tools (shown dimmed)
- ◐ **Running steps** - Currently executing tools (animated spinner)
- ✗ **Failed steps** - Tools that encountered errors (shown in red)

### Step Display Format

Each step shows:
```
[icon] tool_name: description
```

Examples:
```
✓ read_file: Reading src/main.rs
◐ write_file: Creating new module
✗ bash: Running tests
```

### Progress Tracking

The Steps section includes:

- **Progress bar**: Visual indicator of completion (green filled bar)
- **Count display**: Shows `X/Y tools` completed
- **Scroll indicators**: Shows how many steps are above/below the visible area

### Scrolling

The steps list automatically scrolls to show the most recent activity. You can manually scroll through the full history:

- **Alt+↑**: Scroll up (view older steps)
- **Alt+↓**: Scroll down (view newer steps)
- **Auto-reset**: Automatically scrolls to show new steps as they are added

### Scroll Indicators

When there are more steps than can fit on screen:

- Top indicator: `↑ X newer steps (Alt+↑ to scroll)`
- Bottom indicator: `↓ X older steps (Alt+↓ to scroll)`

## Implementation Details

### Data Structure

Tool steps are tracked in `SidebarState`:

```rust
pub struct ToolStepDisplay {
    pub id: String,                          // Unique identifier
    pub tool_name: String,                   // Name of the tool (e.g., "read_file")
    pub description: String,                 // What the tool is doing
    pub status: ToolStepStatus,              // Running/Completed/Failed
    pub timestamp: chrono::DateTime<Local>,  // When the step started
}
```

### Status Icons

```rust
pub enum ToolStepStatus {
    Running,    // ◐ (animated spinner: ◐◓◑◒)
    Completed,  // ✓ (green checkmark)
    Failed,     // ✗ (red X)
}
```

### Key Methods

- `sidebar.add_tool_step(tool_name, description)` - Adds a new running step
- `sidebar.complete_tool_step(tool_name, success)` - Marks a step as complete/failed
- `sidebar.clear_tool_steps()` - Clears all steps (new session/task)
- `sidebar.scroll_tool_steps_up()` - Scroll towards older steps
- `sidebar.scroll_tool_steps_down()` - Scroll towards newer steps
- `sidebar.reset_tool_steps_scroll()` - Jump back to most recent steps

### Event Flow

1. **Tool Start**: `AiUpdate::ToolStart` event triggers `add_tool_step()`
2. **Animation**: Spinner icon rotates through frames: ◐ → ◓ → ◑ → ◒
3. **Tool Complete**: `AiUpdate::ToolComplete` event triggers `complete_tool_step()`
4. **Status Update**: Icon changes to ✓ (success) or ✗ (failure)

## Usage Tips

### Best Practices

1. **Monitor Progress**: Keep an eye on the steps section to see what the AI is doing
2. **Scroll History**: Use Alt+↑/↓ to review previous steps if needed
3. **Identify Issues**: Failed steps (✗) are highlighted in red for quick identification
4. **Context Awareness**: Tool descriptions provide context about each action

### Keyboard Shortcuts Summary

| Shortcut | Action |
|----------|--------|
| `Ctrl+G` | Toggle between Plan/Build mode |
| `Alt+↑` | Scroll steps up (older) |
| `Alt+↓` | Scroll steps down (newer) |
| `Ctrl+B` | Toggle sidebar visibility |

## Visual Example

```
┌─ STEPS ─────────────────┐
│ ████████████░░░░░░░░░░░  │  <- Progress bar
│ 8/14 tools               │  <- Count
│ ↑ 3 newer steps          │  <- Scroll indicator
│ ✓ read_file: main.rs     │  <- Completed (dimmed)
│ ✓ grep: Find functions   │  <- Completed (dimmed)
│ ◐ edit_file: Update...   │  <- Running (animated)
│ ◯ bash: Run tests        │  <- Pending
│ ↓ 6 older steps          │  <- Scroll indicator
└──────────────────────────┘
```

## Benefits

1. **Transparency**: See exactly what the AI is doing in real-time
2. **Debugging**: Quickly identify which tools are failing
3. **Progress Tracking**: Visual feedback on task completion
4. **History**: Scroll back to review all actions taken
5. **Performance**: Know if the AI is stuck or making progress

## Comparison with Plan Mode

| Feature | Plan Mode | Build Mode |
|---------|-----------|------------|
| Display | Todo checklist | Tool execution steps |
| Updates | Task-level progress | Real-time tool tracking |
| Detail | High-level tasks | Low-level tool calls |
| Best For | Understanding the plan | Monitoring execution |

## Integration Points

The Steps feature integrates with:

- **Tool Execution**: Tracks all tool uses via `AiUpdate` events
- **Animation System**: Uses the global animation frame for spinners
- **Sidebar State**: Persists across the session
- **Agent Mode**: Only visible in Build mode

## Future Enhancements

Potential improvements for the Steps feature:

- [ ] Filter steps by tool type
- [ ] Expand/collapse step details
- [ ] Show timing/duration for each step
- [ ] Export step history to file
- [ ] Search through step descriptions
- [ ] Group related steps
- [ ] Step annotations/notes

## Related Files

- `src/tui/sidebar.rs` - Step data structures and state management
- `src/tui/shell_ui.rs` - Step rendering and visualization
- `src/tui/shell_runner.rs` - Event handling and keyboard shortcuts
- `src/tools/mod.rs` - AgentMode enum definition