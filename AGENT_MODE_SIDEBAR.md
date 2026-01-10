# Agent Mode Sidebar Feature

## Overview

The agent mode (PLAN vs BUILD) is now displayed in the right sidebar of the Safe Coder TUI interface, providing clear visual feedback about the current execution mode.

## Implementation Details

### Location
- **File**: `src/tui/shell_ui.rs`
- **Component**: Right sidebar, between TASK and CONTEXT sections

### Visual Layout

The sidebar now includes a new **MODE** section that displays:

```
┌─────────────────────────┐
│  TASK                   │
│  Current task...        │
├─────────────────────────┤
│  MODE                   │
│  BUILD - Full execution │
│  (or)                   │
│  PLAN - Read-only...    │
├─────────────────────────┤
│  CONTEXT                │
│  Token usage info...    │
├─────────────────────────┤
│  FILES                  │
│  Modified files...      │
├─────────────────────────┤
│  PLAN                   │
│  Plan steps...          │
├─────────────────────────┤
│  LSP                    │
│  Connected servers...   │
└─────────────────────────┘
```

### Features

1. **Color-Coded Display**
   - **BUILD mode**: Green (ACCENT_GREEN) - indicates full execution capability
   - **PLAN mode**: Cyan (ACCENT_CYAN) - indicates read-only exploration mode

2. **Description Text**
   - Shows the first sentence of the mode description
   - BUILD: "Full execution mode"
   - PLAN: "Read-only exploration mode"

3. **Real-time Updates**
   - Updates automatically when user cycles through modes using Tab or `/mode` command
   - Synchronized with `app.agent_mode` state

### Code Changes

#### New Function: `draw_sidebar_mode`

```rust
fn draw_sidebar_mode(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        " MODE",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    let mode = app.agent_mode.short_name();
    let description = app.agent_mode.description();

    // Color based on mode
    let mode_color = match mode {
        "BUILD" => ACCENT_GREEN,
        "PLAN" => ACCENT_CYAN,
        _ => TEXT_PRIMARY,
    };

    // Mode display with color
    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("{}", mode),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                " - {}",
                description.split('.').next().unwrap_or(description)
            ),
            Style::default().fg(TEXT_SECONDARY),
        ),
    ]));

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}
```

#### Updated Layout in `draw_sidebar`

```rust
// Sidebar sections: [TASK] [MODE] [CONTEXT] [FILES] [PLAN] [LSP]
let sections = Layout::vertical([
    Constraint::Length(4),               // TASK section
    Constraint::Length(3),               // MODE (agent mode) <- NEW
    Constraint::Length(3),               // CONTEXT (token usage)
    Constraint::Length(modified_height), // FILES (modified files)
    Constraint::Min(6),                  // PLAN (variable height)
    Constraint::Length(5),               // LSP connections
])
.split(inner);

draw_sidebar_task(f, app, sections[0]);
draw_sidebar_mode(f, app, sections[1]);     // <- NEW
draw_sidebar_context(f, app, sections[2]);
draw_sidebar_files(f, app, sections[3]);
draw_sidebar_plan(f, app, sections[4]);
draw_sidebar_lsp(f, app, sections[5]);
```

### Data Flow

1. **Session Level**: `Session.agent_mode` (in `src/session/mod.rs`)
   - Stores the current `AgentMode` enum value
   - Methods: `set_agent_mode()`, `cycle_agent_mode()`

2. **TUI App Level**: `ShellTuiApp.agent_mode` (in `src/tui/shell_app.rs`)
   - Synchronized with session state
   - Accessed by UI rendering code

3. **UI Rendering**: `draw_sidebar_mode()` (in `src/tui/shell_ui.rs`)
   - Reads from `app.agent_mode`
   - Renders visual representation

### User Interactions

Users can change the agent mode using:
- **Ctrl+G**: Keyboard shortcut to cycle between PLAN and BUILD modes
- **`/agent` command**: Slash command to toggle agent mode

The sidebar updates immediately when the mode changes.

Note: The **`/mode`** command is used for toggling permission modes (ASK/EDIT/YOLO), not agent modes.

### AgentMode Enum Reference

From `src/tools/mod.rs`:

```rust
pub enum AgentMode {
    /// Plan mode: Read-only exploration tools only
    Plan,
    /// Build mode: Full tool access including file modifications and bash
    Build,
}
```

**Tool Restrictions:**
- **PLAN mode**: `read_file`, `list_file`, `glob`, `grep`, `ast_grep`, `webfetch`, `todoread`
- **BUILD mode**: All tools including `write_file`, `edit_file`, `bash`, `todowrite`, `build_config`

## Benefits

1. **Visual Clarity**: Users can instantly see which mode they're in
2. **Safety Awareness**: Highlighted mode helps prevent accidental file modifications in PLAN mode
3. **Consistent UX**: Mode is shown in both status bar (bottom) and sidebar (right)
4. **Accessibility**: Color-coded for quick recognition

## Testing

Build the project:
```bash
cargo build
```

Run the application:
```bash
cargo run -- /path/to/project
```

Test mode switching:
1. Press **Ctrl+G** to cycle between PLAN and BUILD modes
2. Type **`/agent`** and press Enter to toggle agent mode
3. Type **`/help`** to see all available commands including agent mode controls
4. Observe the **MODE** section in the right sidebar update in real-time with color-coded display

## Complete Command Reference

### Agent Mode Commands
- **`/agent`** - Toggle between PLAN and BUILD modes
- **Ctrl+G** - Keyboard shortcut to cycle agent modes

### Permission Mode Commands  
- **`/mode`** - Toggle between ASK, EDIT, and YOLO permission modes
- **Ctrl+P** - Keyboard shortcut to cycle permission modes

### Display Locations
1. **Right Sidebar**: MODE section shows current agent mode with color coding
2. **Status Bar**: Bottom right shows current agent mode (BUILD/PLAN)
3. **System Messages**: Mode changes display confirmation messages in the chat

## Implementation Summary

### Files Modified
1. **`src/tui/shell_ui.rs`**
   - Added `draw_sidebar_mode()` function to render MODE section
   - Updated `draw_sidebar()` layout to include MODE section
   - 42 lines added

2. **`src/tui/shell_app.rs`**
   - Added `SlashCommand::Agent` enum variant
   - Updated `parse_slash_command()` to recognize `/agent` command
   - 3 lines added

3. **`src/tui/shell_runner.rs`**
   - Added handler for `SlashCommand::Agent` 
   - Updated help text to include `/agent` command and Ctrl+G shortcut
   - Synchronizes agent mode with session state
   - 25 lines added

### Total Changes
- **70 lines of code added**
- **3 files modified**
- **0 breaking changes**
- **Fully backward compatible**

## Future Enhancements

Potential improvements:
- Add keyboard shortcut hint in MODE section
- Show count of available tools in current mode (e.g., "PLAN - 7 tools")
- Add tooltip with full mode description on hover
- Animate mode transitions with smooth color fades
- Add mode history to track when modes were switched
- Display tool restrictions in a collapsible section