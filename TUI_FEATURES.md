# Safe Coder TUI Features

## Overview

Safe Coder features a beautiful Terminal User Interface (TUI) built with Ratatui, providing a modern, multi-panel layout for interacting with your AI coding assistant.

## Interface Layout

The TUI is divided into four main areas:

### 1. Header (Top)
- Displays the Safe Coder logo and project path
- Shows current context at a glance
- Clean, centered design

### 2. Main Content Area (Middle)

#### Left Panel: Conversation (70% width)
- **Real-time chat** with the AI assistant
- **Message types** with distinct styling:
  - 👤 User messages (blue)
  - 🤖 Assistant responses (green)
  - ℹ️ System messages (yellow)
  - ❌ Error messages (red)
  - 🔧 Tool execution logs (purple)
- **Timestamps** for every message
- **Automatic scrolling** to latest messages
- **Manual scroll** with arrow keys or PageUp/PageDown
- **Text wrapping** for long messages

#### Right Panel: Status & Tools (30% width)

**VM Status (Top)**
- 🟢/🔴 **Running indicator** with color-coded status
- ⏱️ **Uptime counter** in real-time
- 💾 **Memory allocation** display
- ⚙️ **vCPU count** information

**Recent Tools (Bottom)**
- 🔧 **Tool execution history**
- ✓/✗ **Success/failure indicators**
- **Color-coded status**:
  - Green ✓ for successful executions
  - Red ✗ for failed executions
  - Yellow ⏳ for running tools

### 3. Input Area (Bottom-1)
- **Command prompt** with ❯ indicator
- **Live cursor** with animated █ character
- **Backspace support** for editing
- **Thinking indicator** with animated spinner
- **Highlighted border** when active

### 4. Footer (Bottom)
- **Keyboard shortcuts** reference
- **Current status** display
- Quick access to common actions

## Color Scheme

The TUI uses a carefully crafted Catppuccin-inspired color palette:

- **Primary** (Blue): `#8AB4F8` - Actions, highlights, prompts
- **Secondary** (Purple): `#B48EAD` - Tools, accents
- **Success** (Green): `#A6E3A1` - Successful operations
- **Error** (Red): `#F38BA8` - Errors, warnings
- **Warning** (Yellow): `#F9E2AF` - Caution, processing
- **Background**: `#1E1E2E` - Dark, comfortable background
- **Text**: `#CDD6F4` - Readable text color
- **Border**: `#585B70` - Subtle borders

## Keyboard Controls

### Navigation
- `↑` / `↓` - Scroll up/down through conversation
- `PageUp` / `PageDown` - Scroll by full page
- `Tab` - Cycle through panels (Chat → Tools → Status → Chat)

### Input
- `Enter` - Send message / Execute command
- `Backspace` - Delete last character
- Any character - Type message

### Control
- `Ctrl+C` - Exit application gracefully
- `exit` or `quit` - Alternative exit command

## Focus Indicators

The TUI uses visual indicators to show which panel is currently focused:

- **Focused panel**: Bright blue border (`#8AB4F8`)
- **Unfocused panels**: Dim gray border (`#585B70`)
- **Tab key** cycles through panels

## Real-time Updates

The TUI updates in real-time:

1. **VM Status**: Uptime counter updates every second
2. **Spinner**: Animated while AI is thinking (10 frames)
3. **Messages**: Appear instantly as they're received
4. **Tool Execution**: Live status updates

## Animations

### Loading Spinner
When the AI is processing your request:
```
⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
```
Smooth, non-intrusive braille spinner animation.

### Cursor
The input cursor blinks with a solid block character: `█`

## Message Formatting

Messages support:
- **Line wrapping** for long content
- **Timestamps** in HH:MM:SS format
- **Indentation** for multi-line messages
- **Icons** for quick message type identification

## Performance

The TUI is highly optimized:
- **Event polling**: 100ms intervals for responsive input
- **Efficient rendering**: Only redraws when needed
- **Async processing**: Non-blocking message handling
- **Smooth scrolling**: No lag even with long conversations

## Accessibility

- **High contrast** color scheme
- **Clear visual hierarchy**
- **Keyboard-only navigation**
- **Status indicators** for all actions
- **Error messages** are clearly highlighted

## Tips & Tricks

1. **Quick Exit**: Press Ctrl+C at any time to exit
2. **Review History**: Use arrow keys to scroll back through conversation
3. **Monitor Tools**: Switch to Tools panel to see what the AI is executing
4. **Check VM**: The VM status panel shows if something goes wrong
5. **Long Output**: For long AI responses, use PageUp/PageDown for faster scrolling

## Future TUI Enhancements

Planned improvements:
- [ ] Syntax highlighting for code blocks
- [ ] Split view for code editing
- [ ] Search functionality
- [ ] Copy/paste support
- [ ] Theme customization
- [ ] Export conversation to file
- [ ] Minimap for long conversations
- [ ] Tool parameters preview
- [ ] Session history browser
