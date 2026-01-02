# Steps Feature - Visual Guide

## Mode Comparison

### Plan Mode (Ctrl+G to switch)
```
┌─ PLAN ──────────────────┐
│ ████████░░░░░░░░░░░░░░░  │
│ 2/5 tasks                │
│                          │
│ ✓ Analyze requirements   │
│ ✓ Create file structure  │
│ ◐ Implement features     │
│ ◯ Write tests            │
│ ◯ Update documentation   │
│                          │
└──────────────────────────┘
```

### Build Mode (Ctrl+G to switch)
```
┌─ STEPS ─────────────────┐
│ ███████████░░░░░░░░░░░░  │
│ 12/18 tools              │
│                          │
│ ✓ read_file: src/mai...  │
│ ✓ grep: Find function... │
│ ✓ edit_file: Add new...  │
│ ◐ write_file: Create...  │  <- Currently running
│ ↓ 8 older steps          │  <- More below
│                          │
└──────────────────────────┘
```

## Status Icons

### Completed Step (Green, Dimmed)
```
✓ read_file: Reading config.toml
```

### Running Step (Cyan, Animated)
```
◐ edit_file: Updating main.rs
◓ edit_file: Updating main.rs    <- Animates through
◑ edit_file: Updating main.rs       these frames
◒ edit_file: Updating main.rs
```

### Failed Step (Red, Highlighted)
```
✗ bash: Running npm install
```

## Scrolling Behavior

### Auto-Scroll on New Step
When a new tool starts, the view automatically jumps to show it:

```
Before:                      After new step starts:
┌─ STEPS ─────────┐         ┌─ STEPS ─────────┐
│ 5/10 tools       │         │ 5/11 tools       │
│ ↑ 2 newer steps  │         │                  │  <- Auto-scrolled
│ ✓ grep: Search   │         │ ✓ edit_file...   │
│ ✓ read_file...   │         │ ✓ grep: Search   │
│ ✓ bash: Test     │         │ ◐ write_file...  │  <- New step!
│ ↓ 3 older steps  │         │ ↓ 5 older steps  │
└──────────────────┘         └──────────────────┘
```

### Manual Scrolling (Alt+↑/↓)

#### Scroll Up (Alt+↑) - View Older Steps
```
Current view:                After Alt+↑:
┌─ STEPS ─────────┐         ┌─ STEPS ─────────┐
│ 8/15 tools       │         │ 8/15 tools       │
│                  │         │ ↑ 3 newer steps  │  <- Shows newer exist
│ ✓ edit_file...   │         │ ✓ bash: Test     │
│ ✓ grep: Search   │         │ ✓ read_file...   │
│ ◐ write_file...  │         │ ✓ edit_file...   │
│ ↓ 7 older steps  │         │ ↓ 9 older steps  │  <- More below
└──────────────────┘         └──────────────────┘
```

#### Scroll Down (Alt+↓) - View Newer Steps
```
Current view:                After Alt+↓:
┌─ STEPS ─────────┐         ┌─ STEPS ─────────┐
│ 8/15 tools       │         │ 8/15 tools       │
│ ↑ 3 newer steps  │         │ ↑ 1 newer step   │  <- Fewer above
│ ✓ bash: Test     │         │ ✓ grep: Search   │
│ ✓ read_file...   │         │ ◐ write_file...  │
│ ✓ edit_file...   │         │ ◯ bash: Build    │
│ ↓ 9 older steps  │         │ ↓ 7 older steps  │  <- Fewer below
└──────────────────┘         └──────────────────┘
```

## Progress Bar Evolution

### 0% - Just Started
```
┌─ STEPS ─────────────────┐
│ ░░░░░░░░░░░░░░░░░░░░░░░  │
│ 0/20 tools               │
```

### 25% - Making Progress
```
┌─ STEPS ─────────────────┐
│ ██████░░░░░░░░░░░░░░░░░  │
│ 5/20 tools               │
```

### 50% - Halfway
```
┌─ STEPS ─────────────────┐
│ ████████████░░░░░░░░░░░  │
│ 10/20 tools              │
```

### 75% - Almost Done
```
┌─ STEPS ─────────────────┐
│ ██████████████████░░░░░  │
│ 15/20 tools              │
```

### 100% - Complete
```
┌─ STEPS ─────────────────┐
│ ████████████████████████ │
│ 20/20 tools              │
```

## Real-World Example: Building a Feature

### Step 1: Initial Analysis
```
┌─ STEPS ─────────────────┐
│ ████░░░░░░░░░░░░░░░░░░░  │
│ 2/12 tools               │
│                          │
│ ✓ read_file: README.md   │
│ ◐ grep: Find imports     │  <- Analyzing code
│                          │
└──────────────────────────┘
```

### Step 2: Making Changes
```
┌─ STEPS ─────────────────┐
│ ████████░░░░░░░░░░░░░░░  │
│ 5/12 tools               │
│ ↑ 1 newer step           │
│ ✓ read_file: main.rs     │
│ ✓ grep: Find imports     │
│ ◐ edit_file: Add func... │  <- Modifying files
│ ↓ 2 older steps          │
└──────────────────────────┘
```

### Step 3: Testing
```
┌─ STEPS ─────────────────┐
│ ██████████████░░░░░░░░░  │
│ 9/12 tools               │
│ ↑ 4 newer steps          │
│ ✓ write_file: test.rs    │
│ ◐ bash: cargo test       │  <- Running tests
│ ↓ 5 older steps          │
└──────────────────────────┘
```

### Step 4: Complete
```
┌─ STEPS ─────────────────┐
│ ████████████████████████ │
│ 12/12 tools              │
│ ↑ 7 newer steps          │
│ ✓ bash: cargo fmt        │
│ ✓ write_file: docs.md    │
│ ✓ bash: git add .        │
│ ↓ 6 older steps          │
└──────────────────────────┘
```

## Error Handling Example

### When a Step Fails
```
┌─ STEPS ─────────────────┐
│ ████████░░░░░░░░░░░░░░░  │
│ 5/12 tools               │
│                          │
│ ✓ read_file: package.js  │
│ ✓ write_file: index.js   │
│ ✗ bash: npm install      │  <- Failed! (Red)
│ ◐ read_file: Checking... │  <- AI investigating
│                          │
└──────────────────────────┘
```

### After Recovery
```
┌─ STEPS ─────────────────┐
│ ██████████░░░░░░░░░░░░░  │
│ 7/14 tools               │
│ ↑ 1 newer step           │
│ ✗ bash: npm install      │  <- Still visible
│ ✓ write_file: Fix pkg... │  <- Fixed issue
│ ✓ bash: npm install      │  <- Retry succeeded
│ ↓ 3 older steps          │
└──────────────────────────┘
```

## Full Sidebar Context

### Complete Sidebar with Steps
```
┌─ SIDEBAR ────────────────────┐
│                              │
│ ● TASK                       │
│   Implement user auth        │
│                              │
│ ● STEPS                      │
│   ████████████░░░░░░░░░░░░   │
│   8/15 tools                 │
│   ✓ read_file: models/us...  │
│   ✓ grep: Find auth code     │
│   ◐ edit_file: Add JWT...    │
│   ↓ 5 older steps            │
│                              │
│ ● LSP                        │
│   ✓ rust-analyzer            │
│   ✓ typescript-language-...  │
│                              │
│ ● FILES                      │
│   3 modified                 │
│   + src/auth.rs              │
│   ~ src/main.rs              │
│   ~ Cargo.toml               │
│                              │
│ ● TOKENS                     │
│   45.2K / 200K (23%)         │
│                              │
└──────────────────────────────┘
```

## Keyboard Shortcuts Quick Reference

```
┌─ STEPS CONTROLS ─────────────┐
│                              │
│  Ctrl+G    Switch Plan/Build │
│  Alt+↑     Scroll up         │
│  Alt+↓     Scroll down       │
│  Ctrl+B    Toggle sidebar    │
│                              │
└──────────────────────────────┘
```

## Tips & Tricks

### 1. Monitor Long-Running Tasks
```
┌─ STEPS ─────────────────┐
│ ████░░░░░░░░░░░░░░░░░░░  │
│ 1/8 tools                │
│                          │
│ ◐ bash: npm run build    │  <- Watch spinner
│   (Running for 45s...)   │     to know it's alive
│                          │
└──────────────────────────┘
```

### 2. Review After Completion
```
Use Alt+↑ to scroll up and review all steps taken:

┌─ STEPS ─────────────────┐
│ ████████████████████████ │
│ 15/15 tools              │
│ ↑ 12 newer steps         │
│ ✓ read_file: First step  │  <- Scroll to top
│ ✓ grep: Second step      │     to see full
│ ✓ edit_file: Third...   │     history
└──────────────────────────┘
```

### 3. Identify Bottlenecks
```
Look for patterns in failed/slow steps:

┌─ STEPS ─────────────────┐
│ ✗ bash: flaky command    │
│ ✓ bash: retry worked     │
│ ✗ bash: flaky command    │  <- Pattern found!
│ ✓ bash: retry worked     │     Maybe needs fix
└──────────────────────────┘
```

## Animation Frames

The spinner cycles through these Unicode characters at ~4 FPS:

```
Frame 1: ◐
Frame 2: ◓
Frame 3: ◑
Frame 4: ◒
(repeat...)
```

This creates a smooth rotating effect for in-progress steps.

## Color Scheme

- **Green (✓)**: Completed successfully
- **Cyan (◐)**: Currently running
- **Red (✗)**: Failed
- **Dim**: Past completed steps
- **Bright**: Current/recent steps
- **Muted**: Scroll indicators