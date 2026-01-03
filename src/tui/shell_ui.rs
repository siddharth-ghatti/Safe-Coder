//! Shell-first TUI rendering with OpenCode-inspired layout
//!
//! A clean, minimal terminal UI inspired by OpenCode:
//! - Full-width content area (no sidebar)
//! - Status bar at bottom with app info, path, and mode
//! - Clean message blocks with left accent borders
//! - Beautiful side-by-side diff view with line numbers
//! - Simple input with model info

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};
use similar::{ChangeTag, TextDiff};
use textwrap::wrap;

use super::file_picker::FilePicker;
use super::shell_app::{
    BlockOutput, BlockType, CommandBlock, FileDiff, PermissionMode, ShellTuiApp,
};
use super::sidebar::PlanStepDisplay;
use crate::planning::PlanStepStatus;

// ============================================================================
// Color Palette - OpenCode inspired, dark and minimal
// ============================================================================

const ACCENT_CYAN: Color = Color::Rgb(80, 200, 220); // Primary accent (input, selections)
const ACCENT_GREEN: Color = Color::Rgb(120, 200, 120); // Success, additions
const ACCENT_RED: Color = Color::Rgb(220, 100, 100); // Errors, deletions
const ACCENT_YELLOW: Color = Color::Rgb(220, 200, 100); // Warnings, highlights
const ACCENT_MAGENTA: Color = Color::Rgb(180, 120, 200); // AI/model accent
const ACCENT_BLUE: Color = Color::Rgb(100, 140, 200); // Links, info

const TEXT_PRIMARY: Color = Color::Rgb(210, 210, 215); // Main text
const TEXT_SECONDARY: Color = Color::Rgb(150, 150, 160); // Secondary text
const TEXT_DIM: Color = Color::Rgb(100, 100, 110); // Dimmed text
const TEXT_MUTED: Color = Color::Rgb(70, 70, 80); // Very dim text

const BG_PRIMARY: Color = Color::Rgb(30, 32, 40); // Main background
const BG_BLOCK: Color = Color::Rgb(38, 40, 50); // Message block background
const BG_INPUT: Color = Color::Rgb(45, 48, 58); // Input area background
const BG_STATUS: Color = Color::Rgb(35, 38, 48); // Status bar background

const BORDER_SUBTLE: Color = Color::Rgb(55, 58, 68); // Subtle borders
const BORDER_ACCENT: Color = Color::Rgb(80, 200, 220); // Accent borders

// ============================================================================
// Animation Constants
// ============================================================================

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ============================================================================
// Main Draw Function
// ============================================================================

pub fn draw(f: &mut Frame, app: &mut ShellTuiApp) {
    let size = f.area();

    // Fill background
    let bg = Block::default().style(Style::default().bg(BG_PRIMARY));
    f.render_widget(bg, size);

    // Horizontal layout: [main content] [sidebar] (if visible)
    let sidebar_width = if app.sidebar.visible { 28 } else { 0 };

    let horizontal = Layout::horizontal([
        Constraint::Min(40),               // Main content area
        Constraint::Length(sidebar_width), // Sidebar (fixed width)
    ])
    .split(size);

    let main_area = horizontal[0];
    let sidebar_area = horizontal[1];

    // Main content layout: [title] [messages] [input hints] [input] [status bar]
    let chunks = Layout::vertical([
        Constraint::Length(1),                           // Title bar
        Constraint::Min(5),                              // Messages
        Constraint::Length(1),                           // Input hints (enter send / model)
        Constraint::Length(calculate_input_height(app)), // Input area
        Constraint::Length(1),                           // Status bar
    ])
    .split(main_area);

    draw_title_bar(f, app, chunks[0]);
    draw_messages(f, app, chunks[1]);
    draw_input_hints(f, app, chunks[2]);
    draw_input_area(f, app, chunks[3]);
    draw_status_bar(f, app, chunks[4]);

    // Draw sidebar if visible
    if app.sidebar.visible {
        draw_sidebar(f, app, sidebar_area);
    }

    // Popups on top
    if app.file_picker.visible {
        draw_file_picker_popup(f, app, size);
    }

    if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
        draw_autocomplete_popup(f, app, size);
    }

    if app.commands_modal_visible {
        draw_commands_modal(f, app, size);
    }
}

fn calculate_input_height(app: &ShellTuiApp) -> u16 {
    // Estimate wrapped lines (assume ~80 char width, accounting for sidebar)
    let estimated_width = 60usize; // Conservative estimate
    let wrapped_count = if app.input.is_empty() {
        1
    } else {
        // Count wrapped lines
        let mut count = 0;
        for line in app.input.lines() {
            count += ((line.len() / estimated_width) + 1).max(1);
        }
        // Handle case where input has no newlines but is long
        if count == 0 {
            count = ((app.input.len() / estimated_width) + 1).max(1);
        }
        count
    };
    let lines = wrapped_count.min(5) as u16;
    lines + 2 // borders
}

// ============================================================================
// Title Bar
// ============================================================================

fn draw_title_bar(f: &mut Frame, _app: &ShellTuiApp, area: Rect) {
    let title = Paragraph::new(Line::from(vec![Span::styled(
        "safe-coder",
        Style::default()
            .fg(TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(ratatui::layout::Alignment::Center)
    .style(Style::default().bg(BG_PRIMARY));

    f.render_widget(title, area);
}

// ============================================================================
// Messages Area
// ============================================================================

fn draw_messages(f: &mut Frame, app: &mut ShellTuiApp, area: Rect) {
    if area.height < 3 {
        return;
    }

    let content_width = area.width.saturating_sub(4) as usize;

    // Build all rendered lines
    let mut all_lines: Vec<MessageLine> = Vec::new();

    for block in &app.blocks {
        render_block(&mut all_lines, block, content_width, app.animation_frame);
        all_lines.push(MessageLine::Empty); // Gap between blocks
    }

    // Calculate visible portion (auto-scroll to bottom)
    let max_visible = area.height as usize;
    let total_lines = all_lines.len();

    let visible_start = if total_lines > max_visible {
        total_lines
            .saturating_sub(max_visible)
            .saturating_sub(app.scroll_offset)
    } else {
        0
    };
    let visible_end = (visible_start + max_visible).min(total_lines);

    // Render visible lines
    let items: Vec<ListItem> = all_lines
        .get(visible_start..visible_end)
        .unwrap_or(&[])
        .iter()
        .map(|line| ListItem::new(line.to_line()))
        .collect();

    let list = List::new(items).style(Style::default().bg(BG_PRIMARY));
    f.render_widget(list, area);

    // Scrollbar if needed
    if total_lines > max_visible {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some(" "))
            .thumb_symbol("┃")
            .track_style(Style::default().fg(BG_PRIMARY))
            .thumb_style(Style::default().fg(BORDER_SUBTLE));

        let scroll_pos = total_lines
            .saturating_sub(max_visible)
            .saturating_sub(app.scroll_offset);

        let mut state =
            ScrollbarState::new(total_lines.saturating_sub(max_visible)).position(scroll_pos);

        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };

        f.render_stateful_widget(scrollbar, scrollbar_area, &mut state);
    }
}

// ============================================================================
// Message Line Types (OpenCode style)
// ============================================================================

#[derive(Clone)]
enum MessageLine {
    Empty,
    // User input header: "# Change button color..."
    UserHeader {
        text: String,
    },
    // Session link line (dimmed)
    SessionInfo {
        text: String,
    },
    // Block separator
    BlockStart,
    // AI response text with left border
    AiText {
        text: String,
        model: Option<String>,
        timestamp: Option<String>,
    },
    // Tool header: "Edit packages/frontend/..."
    ToolHeader {
        tool: String,
        target: String,
    },
    // Diff line with line numbers
    DiffContext {
        old_num: String,
        new_num: String,
        text: String,
    },
    DiffRemove {
        old_num: String,
        text: String,
    },
    DiffAdd {
        new_num: String,
        text: String,
    },
    // Shell command output
    ShellOutput {
        text: String,
    },
    // System/info message
    SystemInfo {
        text: String,
    },
    // Streaming/running indicator
    Running {
        text: String,
        spinner_frame: usize,
    },
}

impl MessageLine {
    fn to_line(&self) -> Line<'static> {
        match self {
            MessageLine::Empty => Line::from(""),

            MessageLine::UserHeader { text } => Line::from(vec![
                Span::styled("# ", Style::default().fg(TEXT_DIM)),
                Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(TEXT_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),

            MessageLine::SessionInfo { text } => {
                Line::from(Span::styled(text.clone(), Style::default().fg(TEXT_DIM)))
            }

            MessageLine::BlockStart => Line::from(Span::styled(
                "─".repeat(60),
                Style::default().fg(BORDER_SUBTLE),
            )),

            MessageLine::AiText {
                text,
                model,
                timestamp,
            } => {
                let mut spans = vec![
                    Span::styled("│ ", Style::default().fg(BORDER_ACCENT)),
                    Span::styled(text.clone(), Style::default().fg(TEXT_PRIMARY)),
                ];

                // Add model/timestamp on first line if present
                if let (Some(m), Some(t)) = (model, timestamp) {
                    spans.push(Span::styled(
                        format!("\n  {} ({})", m, t),
                        Style::default().fg(TEXT_DIM),
                    ));
                }

                Line::from(spans)
            }

            MessageLine::ToolHeader { tool, target } => Line::from(vec![
                Span::styled("│ ", Style::default().fg(BORDER_SUBTLE)),
                Span::styled(format!("{} ", tool), Style::default().fg(TEXT_SECONDARY)),
                Span::styled(target.clone(), Style::default().fg(TEXT_PRIMARY)),
            ]),

            MessageLine::DiffContext {
                old_num,
                new_num,
                text,
            } => Line::from(vec![
                Span::styled("│ ", Style::default().fg(BORDER_SUBTLE)),
                Span::styled(format!("{:>4} ", old_num), Style::default().fg(TEXT_DIM)),
                Span::styled(format!("{:>4} ", new_num), Style::default().fg(TEXT_DIM)),
                Span::styled("  ", Style::default()),
                Span::styled(text.clone(), Style::default().fg(TEXT_SECONDARY)),
            ]),

            MessageLine::DiffRemove { old_num, text } => Line::from(vec![
                Span::styled("│ ", Style::default().fg(ACCENT_RED)),
                Span::styled(format!("{:>4} ", old_num), Style::default().fg(ACCENT_RED)),
                Span::styled("     ", Style::default()),
                Span::styled("- ", Style::default().fg(ACCENT_RED)),
                Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(ACCENT_RED)
                        .add_modifier(Modifier::CROSSED_OUT),
                ),
            ]),

            MessageLine::DiffAdd { new_num, text } => Line::from(vec![
                Span::styled("│ ", Style::default().fg(ACCENT_GREEN)),
                Span::styled("     ", Style::default()),
                Span::styled(
                    format!("{:>4} ", new_num),
                    Style::default().fg(ACCENT_GREEN),
                ),
                Span::styled("+ ", Style::default().fg(ACCENT_GREEN)),
                Span::styled(text.clone(), Style::default().fg(ACCENT_GREEN)),
            ]),

            MessageLine::ShellOutput { text } => Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(text.clone(), Style::default().fg(TEXT_SECONDARY)),
            ]),

            MessageLine::SystemInfo { text } => {
                Line::from(Span::styled(text.clone(), Style::default().fg(TEXT_DIM)))
            }

            MessageLine::Running {
                text,
                spinner_frame,
            } => {
                let spinner = SPINNER_FRAMES[*spinner_frame % SPINNER_FRAMES.len()];
                Line::from(vec![
                    Span::styled("│ ", Style::default().fg(ACCENT_CYAN)),
                    Span::styled(format!("{} ", spinner), Style::default().fg(ACCENT_CYAN)),
                    Span::styled(text.clone(), Style::default().fg(TEXT_SECONDARY)),
                ])
            }
        }
    }
}

// ============================================================================
// Block Rendering
// ============================================================================

fn render_block(lines: &mut Vec<MessageLine>, block: &CommandBlock, width: usize, frame: usize) {
    match &block.block_type {
        BlockType::SystemMessage => {
            for line in block.output.get_text().lines() {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::SystemInfo {
                        text: wrapped.to_string(),
                    });
                }
            }
        }

        BlockType::ShellCommand => {
            // User command header - wrap long commands
            for wrapped in wrap(&block.input, width.saturating_sub(4)) {
                lines.push(MessageLine::UserHeader {
                    text: wrapped.to_string(),
                });
            }

            if block.is_running() {
                lines.push(MessageLine::Running {
                    text: "Running...".to_string(),
                    spinner_frame: frame,
                });
            }

            // Output
            render_output(lines, &block.output, width);
        }

        BlockType::AiQuery => {
            // User query header (like "# Change button color to danger...") - wrap long queries
            for wrapped in wrap(&block.input, width.saturating_sub(4)) {
                lines.push(MessageLine::UserHeader {
                    text: wrapped.to_string(),
                });
            }

            if block.is_running() && block.children.is_empty() && block.output.get_text().is_empty()
            {
                lines.push(MessageLine::Running {
                    text: "Thinking...".to_string(),
                    spinner_frame: frame,
                });
            }

            // Render children (tools, reasoning)
            for child in &block.children {
                render_child_block(lines, child, width, frame);
            }

            // Final AI response
            let text = block.output.get_text();
            if !text.is_empty() {
                lines.push(MessageLine::Empty);
                lines.push(MessageLine::BlockStart);

                let output_lines: Vec<&str> = text.lines().collect();
                let total = output_lines.len();

                for (i, line) in output_lines.iter().enumerate() {
                    for wrapped in wrap(line, width.saturating_sub(4)) {
                        let is_last = i == total - 1;
                        lines.push(MessageLine::AiText {
                            text: wrapped.to_string(),
                            model: if is_last {
                                Some("claude-sonnet-4".to_string())
                            } else {
                                None
                            },
                            timestamp: if is_last {
                                Some(chrono::Local::now().format("%I:%M %p").to_string())
                            } else {
                                None
                            },
                        });
                    }
                }
            }
        }

        BlockType::Orchestration => {
            lines.push(MessageLine::UserHeader {
                text: format!("orchestrate: {}", block.input),
            });

            render_output(lines, &block.output, width);

            for child in &block.children {
                render_child_block(lines, child, width, frame);
            }
        }

        BlockType::AiToolExecution { .. } => {
            render_child_block(lines, block, width, frame);
        }

        BlockType::AiReasoning => {
            let text = block.output.get_text();
            for line in text.lines() {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::AiText {
                        text: wrapped.to_string(),
                        model: None,
                        timestamp: None,
                    });
                }
            }
        }

        BlockType::Subagent { kind } => {
            // Render subagent like an AI tool execution
            lines.push(MessageLine::UserHeader {
                text: format!("Subagent: {} - {}", kind, block.input),
            });

            render_output(lines, &block.output, width);

            for child in &block.children {
                render_child_block(lines, child, width, frame);
            }
        }
    }
}

fn render_child_block(
    lines: &mut Vec<MessageLine>,
    block: &CommandBlock,
    width: usize,
    frame: usize,
) {
    match &block.block_type {
        BlockType::AiToolExecution { tool_name } => {
            lines.push(MessageLine::Empty);
            lines.push(MessageLine::BlockStart);

            // Tool header
            let target = if !block.input.is_empty() {
                block.input.clone()
            } else {
                String::new()
            };

            // Capitalize first letter of tool name
            let tool_display = match tool_name.as_str() {
                "bash" => "Bash".to_string(),
                "read" | "Read" => "Read".to_string(),
                "write" | "Write" => "Write".to_string(),
                "edit" | "Edit" => "Edit".to_string(),
                "glob" | "Glob" => "Glob".to_string(),
                "grep" | "Grep" => "Grep".to_string(),
                name if name.starts_with("task-") => format!("Task {}", &name[5..]),
                other => other.to_string(),
            };

            lines.push(MessageLine::ToolHeader {
                tool: tool_display,
                target,
            });

            if block.is_running() {
                lines.push(MessageLine::Running {
                    text: "Executing...".to_string(),
                    spinner_frame: frame,
                });
            }

            // Show diff if present (OpenCode-style with line numbers)
            if let Some(diff) = &block.diff {
                render_diff_opencode(lines, diff, width);
            } else if tool_name == "bash"
                || tool_name.starts_with("task-")
                || tool_name.starts_with("subagent:")
            {
                render_tool_output(lines, &block.output, width);
            }
        }

        BlockType::AiReasoning => {
            let text = block.output.get_text();
            if !text.is_empty() {
                lines.push(MessageLine::Empty);
                for line in text.lines() {
                    for wrapped in wrap(line, width.saturating_sub(4)) {
                        lines.push(MessageLine::AiText {
                            text: wrapped.to_string(),
                            model: None,
                            timestamp: None,
                        });
                    }
                }
            }
        }

        _ => {}
    }
}

fn render_output(lines: &mut Vec<MessageLine>, output: &BlockOutput, width: usize) {
    match output {
        BlockOutput::Streaming {
            lines: output_lines,
            ..
        } => {
            for line in output_lines.iter().take(20) {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::ShellOutput {
                        text: wrapped.to_string(),
                    });
                }
            }
            if output_lines.len() > 20 {
                lines.push(MessageLine::ShellOutput {
                    text: format!("... {} more lines", output_lines.len() - 20),
                });
            }
        }
        BlockOutput::Success(text) if !text.is_empty() => {
            for line in text.lines().take(20) {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::ShellOutput {
                        text: wrapped.to_string(),
                    });
                }
            }
            if text.lines().count() > 20 {
                lines.push(MessageLine::ShellOutput {
                    text: format!("... {} more lines", text.lines().count() - 20),
                });
            }
        }
        BlockOutput::Error { message, .. } => {
            lines.push(MessageLine::ShellOutput {
                text: format!("Error: {}", message),
            });
        }
        _ => {}
    }
}

fn render_tool_output(lines: &mut Vec<MessageLine>, output: &BlockOutput, width: usize) {
    match output {
        BlockOutput::Streaming {
            lines: output_lines,
            ..
        } => {
            // Show more lines for subagent streaming output
            let max_lines = 50;
            for line in output_lines.iter().take(max_lines) {
                for wrapped in wrap(line, width.saturating_sub(14)) {
                    lines.push(MessageLine::ShellOutput {
                        text: format!("  {}", wrapped),
                    });
                }
            }
            if output_lines.len() > max_lines {
                lines.push(MessageLine::ShellOutput {
                    text: format!("  ... {} more lines", output_lines.len() - max_lines),
                });
            }
        }
        BlockOutput::Success(text) if !text.is_empty() => {
            let max_lines = 30;
            let text_lines: Vec<&str> = text.lines().collect();
            for line in text_lines.iter().take(max_lines) {
                for wrapped in wrap(line, width.saturating_sub(14)) {
                    lines.push(MessageLine::ShellOutput {
                        text: format!("  {}", wrapped),
                    });
                }
            }
            if text_lines.len() > max_lines {
                lines.push(MessageLine::ShellOutput {
                    text: format!("  ... {} more lines", text_lines.len() - max_lines),
                });
            }
        }
        _ => {}
    }
}

/// Render diff in OpenCode style with side-by-side line numbers
fn render_diff_opencode(lines: &mut Vec<MessageLine>, diff: &FileDiff, _width: usize) {
    let text_diff = TextDiff::from_lines(&diff.old_content, &diff.new_content);

    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut changes_shown = 0;
    let max_changes = 15;

    // Find the first change and show some context
    let changes: Vec<_> = text_diff.iter_all_changes().collect();

    // Find first actual change
    let first_change_idx = changes.iter().position(|c| c.tag() != ChangeTag::Equal);
    let start_idx = first_change_idx.map(|i| i.saturating_sub(2)).unwrap_or(0);

    for (idx, change) in changes.iter().enumerate().skip(start_idx) {
        if changes_shown >= max_changes {
            break;
        }

        let content = change.value().trim_end();

        match change.tag() {
            ChangeTag::Equal => {
                // Only show context around changes
                let near_change = idx >= start_idx && idx < start_idx + max_changes + 4;
                if near_change {
                    lines.push(MessageLine::DiffContext {
                        old_num: old_line.to_string(),
                        new_num: new_line.to_string(),
                        text: content.to_string(),
                    });
                }
                old_line += 1;
                new_line += 1;
            }
            ChangeTag::Delete => {
                lines.push(MessageLine::DiffRemove {
                    old_num: old_line.to_string(),
                    text: content.to_string(),
                });
                old_line += 1;
                changes_shown += 1;
            }
            ChangeTag::Insert => {
                lines.push(MessageLine::DiffAdd {
                    new_num: new_line.to_string(),
                    text: content.to_string(),
                });
                new_line += 1;
                changes_shown += 1;
            }
        }
    }

    let total_changes = text_diff
        .iter_all_changes()
        .filter(|c| c.tag() != ChangeTag::Equal)
        .count();

    if total_changes > max_changes {
        lines.push(MessageLine::ShellOutput {
            text: format!("  ... {} more changes", total_changes - max_changes),
        });
    }
}

// ============================================================================
// Input Hints (above input)
// ============================================================================

fn draw_input_hints(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let model_name = if app.ai_connected {
        "Anthropic Claude Sonnet 4"
    } else {
        "Not connected"
    };

    let left = Span::styled("enter ", Style::default().fg(TEXT_DIM));
    let left2 = Span::styled("send", Style::default().fg(TEXT_SECONDARY));

    let right = Span::styled(model_name, Style::default().fg(TEXT_SECONDARY));

    // Calculate spacing
    let left_len = 10; // "enter send"
    let right_len = model_name.len();
    let padding = area
        .width
        .saturating_sub(left_len as u16 + right_len as u16 + 2) as usize;

    let line = Line::from(vec![
        Span::styled(" ", Style::default()),
        left,
        left2,
        Span::styled(" ".repeat(padding), Style::default()),
        right,
        Span::styled(" ", Style::default()),
    ]);

    let para = Paragraph::new(line).style(Style::default().bg(BG_PRIMARY));
    f.render_widget(para, area);
}

// ============================================================================
// Input Area
// ============================================================================

fn draw_input_area(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_SUBTLE))
        .style(Style::default().bg(BG_INPUT));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let available_width = inner.width.saturating_sub(3) as usize; // Account for "> " prefix
    let available_height = inner.height as usize;

    // Blinking cursor
    let cursor_visible = app.animation_frame % 16 < 10;
    let cursor_char = if cursor_visible { "█" } else { " " };

    if app.input.is_empty() {
        // Show placeholder when empty
        let spans = vec![
            Span::styled("> ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                cursor_char.to_string(),
                Style::default()
                    .fg(ACCENT_CYAN)
                    .add_modifier(Modifier::REVERSED),
            ),
            Span::styled("Type a message...", Style::default().fg(TEXT_MUTED)),
        ];
        let para = Paragraph::new(Line::from(spans));
        f.render_widget(para, inner);
        return;
    }

    // For non-empty input, we need to handle wrapping manually to track cursor position
    let input_with_cursor = format!(
        "{}{}{}",
        &app.input[..app.cursor_pos.min(app.input.len())],
        "\x00", // Cursor marker
        &app.input[app.cursor_pos.min(app.input.len())..]
    );

    // Wrap the text with cursor marker
    let wrapped_lines: Vec<String> = wrap(&input_with_cursor, available_width)
        .into_iter()
        .map(|cow| cow.to_string())
        .collect();

    // Find which line contains the cursor and scroll to show it
    let mut cursor_line = 0;
    for (i, line) in wrapped_lines.iter().enumerate() {
        if line.contains('\x00') {
            cursor_line = i;
            break;
        }
    }

    // Calculate visible range to keep cursor in view
    let visible_start = if cursor_line >= available_height {
        cursor_line - available_height + 1
    } else {
        0
    };
    let visible_end = (visible_start + available_height).min(wrapped_lines.len());

    // Build the display lines
    let mut lines: Vec<Line> = Vec::new();
    for (i, line) in wrapped_lines
        .iter()
        .enumerate()
        .skip(visible_start)
        .take(visible_end - visible_start)
    {
        let prefix = if i == 0 { "> " } else { "  " };

        if line.contains('\x00') {
            // This line contains the cursor
            let parts: Vec<&str> = line.splitn(2, '\x00').collect();
            let before = parts.get(0).unwrap_or(&"");
            let after = parts.get(1).unwrap_or(&"");

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(TEXT_DIM)),
                Span::styled(before.to_string(), Style::default().fg(TEXT_PRIMARY)),
                Span::styled(
                    cursor_char.to_string(),
                    Style::default()
                        .fg(ACCENT_CYAN)
                        .add_modifier(Modifier::REVERSED),
                ),
                Span::styled(after.to_string(), Style::default().fg(TEXT_PRIMARY)),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(TEXT_DIM)),
                Span::styled(line.clone(), Style::default().fg(TEXT_PRIMARY)),
            ]));
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, inner);
}

// ============================================================================
// Status Bar (bottom)
// ============================================================================

fn draw_status_bar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Get path, truncate if needed
    let path = app
        .cwd
        .to_string_lossy()
        .replace(&std::env::var("HOME").unwrap_or_default(), "~");

    let mode = app.agent_mode.short_name();
    let version = "v0.1.0";

    // Build spans (removed LSP status - it's shown in sidebar instead)
    let mut spans = vec![
        Span::styled(" ", Style::default()),
        Span::styled(
            "safe-coder",
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}  ", version), Style::default().fg(TEXT_DIM)),
        Span::styled(path.clone(), Style::default().fg(TEXT_SECONDARY)),
    ];

    // Calculate padding for right side
    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let right_text = format!("tab  {} MODE", mode.to_uppercase());
    let padding = area
        .width
        .saturating_sub(left_len as u16 + right_text.len() as u16 + 2) as usize;

    spans.push(Span::styled(" ".repeat(padding.max(1)), Style::default()));
    spans.push(Span::styled("tab", Style::default().fg(TEXT_DIM)));
    spans.push(Span::styled("  ", Style::default()));
    spans.push(Span::styled(
        format!("{} MODE", mode.to_uppercase()),
        Style::default()
            .fg(TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(" ", Style::default()));

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(BG_STATUS));
    f.render_widget(para, area);
}

// ============================================================================
// Sidebar (OpenCode-style)
// ============================================================================

fn draw_sidebar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Sidebar background with left border
    let sidebar_block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(BORDER_SUBTLE))
        .style(Style::default().bg(BG_PRIMARY));

    let inner = sidebar_block.inner(area);
    f.render_widget(sidebar_block, area);

    // Calculate modified files section height (dynamic based on file count)
    let modified_count = app.sidebar.modified_files.len();
    let modified_height = if modified_count == 0 {
        2 // Just header + "No changes"
    } else {
        (modified_count.min(5) + 2) as u16 // Header + files (max 5) + potential overflow
    };

    // Sidebar sections: [TASK] [CONTEXT] [FILES] [PLAN] [LSP]
    let sections = Layout::vertical([
        Constraint::Length(4),               // TASK section
        Constraint::Length(3),               // CONTEXT (token usage)
        Constraint::Length(modified_height), // FILES (modified files)
        Constraint::Min(6),                  // PLAN (variable height)
        Constraint::Length(5),               // LSP connections
    ])
    .split(inner);

    draw_sidebar_task(f, app, sections[0]);
    draw_sidebar_context(f, app, sections[1]);
    draw_sidebar_files(f, app, sections[2]);
    draw_sidebar_plan(f, app, sections[3]);
    draw_sidebar_lsp(f, app, sections[4]);
}

fn draw_sidebar_task(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        " TASK",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    if let Some(ref task) = app.sidebar.current_task {
        // Truncate task if too long
        let max_len = area.width.saturating_sub(4) as usize; // Leave room for spinner
        let display = if task.len() > max_len {
            format!("{}...", &task[..max_len.saturating_sub(3)])
        } else {
            task.clone()
        };

        // Show animated spinner when AI is thinking
        if app.ai_thinking {
            let spinner_chars = ["◐", "◓", "◑", "◒"];
            let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(format!("{} ", spinner), Style::default().fg(ACCENT_CYAN)),
                Span::styled(display, Style::default().fg(TEXT_PRIMARY)),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!(" {}", display),
                Style::default().fg(TEXT_PRIMARY),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            " No active task",
            Style::default().fg(TEXT_MUTED),
        )));
    }

    // Show step description if in progress
    if let Some(ref plan) = app.sidebar.active_plan {
        if let Some(ref desc) = plan.current_step_description {
            let max_len = area.width.saturating_sub(2) as usize;
            let display = if desc.len() > max_len {
                format!("{}...", &desc[..max_len.saturating_sub(3)])
            } else {
                desc.clone()
            };
            lines.push(Line::from(Span::styled(
                format!(" {}", display),
                Style::default().fg(ACCENT_CYAN),
            )));
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

fn draw_sidebar_context(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let usage = &app.sidebar.token_usage;

    let mut lines = vec![Line::from(Span::styled(
        " CONTEXT",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    // Token count - show live tokens and compressed tokens if any
    if usage.compressed_tokens > 0 {
        lines.push(Line::from(Span::styled(
            format!(
                " {} (+{} compressed)",
                usage.format_display(),
                format_number(usage.compressed_tokens)
            ),
            Style::default().fg(TEXT_SECONDARY),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(" {}", usage.format_display()),
            Style::default().fg(TEXT_SECONDARY),
        )));
    }

    // Cache statistics - only show if there's cache activity
    if usage.has_cache_activity() {
        lines.push(Line::from(Span::styled(
            format!(" {}", usage.format_cache_display()),
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(Span::styled(
            format!(" {}", usage.format_savings()),
            Style::default().fg(Color::Green),
        )));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Format large numbers with K/M suffixes (duplicated here for shell_ui)
fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn draw_sidebar_files(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    use super::sidebar::ModificationType;

    let mut lines = vec![Line::from(Span::styled(
        " FILES",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    let files = &app.sidebar.modified_files;

    if files.is_empty() {
        lines.push(Line::from(Span::styled(
            " No changes",
            Style::default().fg(TEXT_MUTED),
        )));
    } else {
        let max_files = 5;
        for file in files.iter().take(max_files) {
            // Icon and color based on modification type
            let (icon, color) = match file.modification_type {
                ModificationType::Created => ("+", ACCENT_GREEN),
                ModificationType::Edited => ("~", ACCENT_YELLOW),
                ModificationType::Deleted => ("-", ACCENT_RED),
            };

            // Get just the filename, not the full path
            let filename = std::path::Path::new(&file.path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| file.path.clone());

            // Truncate if too long
            let max_len = area.width.saturating_sub(5) as usize;
            let display = if filename.len() > max_len {
                format!("{}...", &filename[..max_len.saturating_sub(3)])
            } else {
                filename
            };

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(display, Style::default().fg(TEXT_SECONDARY)),
            ]));
        }

        if files.len() > max_files {
            lines.push(Line::from(Span::styled(
                format!(" ... {} more", files.len() - max_files),
                Style::default().fg(TEXT_MUTED),
            )));
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

fn draw_sidebar_plan(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Check if we're in build mode to show tool steps instead of todo plan
    let show_tool_steps = app.agent_mode == crate::tools::AgentMode::Build;

    let mut lines = vec![Line::from(Span::styled(
        if show_tool_steps { " STEPS" } else { " PLAN" },
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    // In build mode, show tool execution steps
    if show_tool_steps && !app.sidebar.tool_steps.is_empty() {
        let tool_steps = &app.sidebar.tool_steps;

        // Show progress bar based on completed vs total tool steps
        let completed_count = app.sidebar.completed_tool_steps();
        let total_count = tool_steps.len();
        let percent = if total_count > 0 {
            (completed_count as f32 / total_count as f32) * 100.0
        } else {
            0.0
        };

        let bar_width = area.width.saturating_sub(4) as usize;
        let filled = ((percent / 100.0) * bar_width as f32) as usize;
        let empty = bar_width.saturating_sub(filled);

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("█".repeat(filled), Style::default().fg(ACCENT_GREEN)),
            Span::styled("░".repeat(empty), Style::default().fg(TEXT_MUTED)),
        ]));

        // Show step count
        let progress = format!(" {}/{} tools", completed_count, total_count);
        lines.push(Line::from(Span::styled(
            progress,
            Style::default().fg(TEXT_SECONDARY),
        )));

        // Show recent tool steps (max items based on available height)
        let max_items = area.height.saturating_sub(4) as usize;
        let scroll_offset = app.sidebar.tool_steps_scroll_offset;
        let total_steps = tool_steps.len();

        // Calculate visible range with scroll offset
        let visible_steps: Vec<_> = tool_steps
            .iter()
            .rev() // Show most recent first (index 0 in reversed list is most recent)
            .skip(scroll_offset)
            .take(max_items)
            .collect();

        // Show scroll indicator at top if scrolled down (showing older items)
        if scroll_offset > 0 {
            lines.push(Line::from(Span::styled(
                format!(
                    " ↑ {} newer steps (Alt+↑ to scroll)",
                    scroll_offset.min(total_steps)
                ),
                Style::default().fg(TEXT_MUTED),
            )));
        }

        for step in visible_steps.iter() {
            // Use animated spinner for running steps
            let (icon, icon_color) = match step.status {
                crate::tui::sidebar::ToolStepStatus::Completed => ("✓".to_string(), ACCENT_GREEN),
                crate::tui::sidebar::ToolStepStatus::Running => {
                    let spinner_chars = ["◐", "◓", "◑", "◒"];
                    let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
                    (spinner.to_string(), ACCENT_CYAN)
                }
                crate::tui::sidebar::ToolStepStatus::Failed => ("✗".to_string(), ACCENT_RED),
            };

            // Format tool name and description
            let display_text = if step.description.is_empty() {
                step.tool_name.clone()
            } else {
                format!("{}: {}", step.tool_name, step.description)
            };

            // Truncate if too long
            let max_len = area.width.saturating_sub(5) as usize;
            let desc = if display_text.len() > max_len {
                format!("{}...", &display_text[..max_len.saturating_sub(3)])
            } else {
                display_text
            };

            // Style based on status
            let desc_style = match step.status {
                crate::tui::sidebar::ToolStepStatus::Running => Style::default().fg(TEXT_PRIMARY),
                crate::tui::sidebar::ToolStepStatus::Completed => Style::default().fg(TEXT_DIM),
                crate::tui::sidebar::ToolStepStatus::Failed => Style::default().fg(ACCENT_RED),
            };

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
                Span::styled(desc, desc_style),
            ]));
        }

        // Show scroll indicator at bottom if there are more older steps
        let items_below = total_steps.saturating_sub(scroll_offset + visible_steps.len());
        if items_below > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↓ {} older steps (Alt+↓ to scroll)", items_below),
                Style::default().fg(TEXT_MUTED),
            )));
        }
    }
    // In plan mode or when no tool steps, show todo plan if available
    else if let Some(ref todo_plan) = app.sidebar.todo_plan {
        // Show progress bar
        let percent = todo_plan.progress_percent();
        let bar_width = area.width.saturating_sub(4) as usize;
        let filled = ((percent / 100.0) * bar_width as f32) as usize;
        let empty = bar_width.saturating_sub(filled);

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled("█".repeat(filled), Style::default().fg(ACCENT_GREEN)),
            Span::styled("░".repeat(empty), Style::default().fg(TEXT_MUTED)),
        ]));

        // Show item count
        let progress = format!(
            " {}/{} tasks",
            todo_plan.completed_count(),
            todo_plan.items.len()
        );
        lines.push(Line::from(Span::styled(
            progress,
            Style::default().fg(TEXT_SECONDARY),
        )));

        // Calculate visible items - scroll to show in_progress item or most recent
        let max_items = area.height.saturating_sub(4) as usize;
        let total_items = todo_plan.items.len();

        // Find the in_progress item index, or default to showing from the end
        let in_progress_idx = todo_plan
            .items
            .iter()
            .position(|i| i.status == "in_progress");

        // Calculate scroll offset to keep in_progress item visible, or show latest items
        let scroll_start = if let Some(idx) = in_progress_idx {
            // Center the in-progress item if possible
            if total_items <= max_items {
                0
            } else if idx < max_items / 2 {
                0
            } else if idx > total_items - max_items / 2 {
                total_items.saturating_sub(max_items)
            } else {
                idx.saturating_sub(max_items / 2)
            }
        } else {
            // No in-progress item, show from start (completed items first)
            0
        };

        let visible_items: Vec<_> = todo_plan
            .items
            .iter()
            .skip(scroll_start)
            .take(max_items)
            .collect();

        // Show scroll indicator at top if needed
        if scroll_start > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↑ {} more above", scroll_start),
                Style::default().fg(TEXT_MUTED),
            )));
        }

        // Show each visible todo item with status icon
        for item in visible_items.iter() {
            // Use animated spinner for in-progress items
            let (icon, icon_color) = match item.status.as_str() {
                "completed" => ("✓".to_string(), ACCENT_GREEN),
                "in_progress" => {
                    let spinner_chars = ["◐", "◓", "◑", "◒"];
                    let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
                    (spinner.to_string(), ACCENT_CYAN)
                }
                "pending" => ("◯".to_string(), TEXT_DIM),
                _ => ("?".to_string(), TEXT_MUTED),
            };

            // Truncate item content
            let max_len = area.width.saturating_sub(5) as usize;
            let desc = if item.content.len() > max_len {
                format!("{}...", &item.content[..max_len.saturating_sub(3)])
            } else {
                item.content.clone()
            };

            // Highlight in-progress item, dim completed items
            let desc_style = match item.status.as_str() {
                "in_progress" => Style::default().fg(TEXT_PRIMARY),
                "completed" => Style::default().fg(TEXT_DIM),
                _ => Style::default().fg(TEXT_SECONDARY),
            };

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
                Span::styled(desc, desc_style),
            ]));
        }

        // Show scroll indicator at bottom if needed
        let items_below = total_items.saturating_sub(scroll_start + visible_items.len());
        if items_below > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↓ {} more below", items_below),
                Style::default().fg(TEXT_MUTED),
            )));
        }
    } else if app.ai_thinking {
        // Show thinking spinner when AI is processing
        let spinner_chars = ["◐", "◓", "◑", "◒"];
        let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("{} ", spinner), Style::default().fg(ACCENT_CYAN)),
            Span::styled("Thinking...", Style::default().fg(TEXT_SECONDARY)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            if show_tool_steps {
                " No steps"
            } else {
                " No tasks"
            },
            Style::default().fg(TEXT_MUTED),
        )));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

fn draw_sidebar_lsp(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        " LSP",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    let connections = &app.sidebar.connections;

    if connections.lsp_servers.is_empty() {
        if app.lsp_initializing {
            let spinner_chars = ["◐", "◓", "◑", "◒"];
            let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
            lines.push(Line::from(Span::styled(
                format!(" {} Initializing...", spinner),
                Style::default().fg(TEXT_DIM),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " No servers",
                Style::default().fg(TEXT_MUTED),
            )));
        }
    } else {
        for (name, connected) in &connections.lsp_servers {
            let (icon, color) = if *connected {
                ("●", ACCENT_GREEN)
            } else {
                ("○", ACCENT_RED)
            };

            // Truncate server name if needed
            let max_len = area.width.saturating_sub(5) as usize;
            let display = if name.len() > max_len {
                format!("{}...", &name[..max_len.saturating_sub(3)])
            } else {
                name.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(display, Style::default().fg(TEXT_SECONDARY)),
            ]));
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

// ============================================================================
// Popups
// ============================================================================

fn draw_file_picker_popup(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let filtered = app.file_picker.filtered_entries();
    if filtered.is_empty() && app.file_picker.filter.is_empty() {
        return;
    }

    let max_entries = 10;
    let height = (filtered.len().min(max_entries) + 3) as u16;
    let width = 50.min(area.width.saturating_sub(10));

    let popup_area = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: area.height.saturating_sub(height + 6),
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .title(" Files ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Filter input
    let filter_area = Rect { height: 1, ..inner };
    let filter_text = if app.file_picker.filter.is_empty() {
        "Type to filter...".to_string()
    } else {
        app.file_picker.filter.clone()
    };

    let filter_para = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(ACCENT_CYAN)),
        Span::styled(
            filter_text,
            if app.file_picker.filter.is_empty() {
                Style::default().fg(TEXT_MUTED)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            },
        ),
    ]));
    f.render_widget(filter_para, filter_area);

    // File list
    let list_area = Rect {
        y: inner.y + 1,
        height: inner.height.saturating_sub(1),
        ..inner
    };

    if filtered.is_empty() {
        let no_match = Paragraph::new("No matches").style(Style::default().fg(TEXT_MUTED));
        f.render_widget(no_match, list_area);
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .take(max_entries)
        .map(|(i, entry)| {
            let style = if i == app.file_picker.selected {
                Style::default()
                    .fg(BG_PRIMARY)
                    .bg(ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };

            let icon = if entry.is_dir { "📁" } else { "📄" };
            ListItem::new(format!("{} {}", icon, entry.name)).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, list_area);
}

fn draw_autocomplete_popup(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let suggestions = &app.autocomplete.suggestions;
    if suggestions.is_empty() {
        return;
    }

    let width = suggestions.iter().map(|s| s.len()).max().unwrap_or(10) as u16 + 6;
    let width = width.min(40);
    let height = (suggestions.len() as u16 + 2).min(10);

    let popup_area = Rect {
        x: 4,
        y: area.height.saturating_sub(height + 6),
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .take(8)
        .map(|(i, s)| {
            let style = if i == app.autocomplete.selected {
                Style::default()
                    .fg(BG_PRIMARY)
                    .bg(ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };
            ListItem::new(format!("  {}", s)).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// Draw the commands reference modal
fn draw_commands_modal(f: &mut Frame, _app: &ShellTuiApp, area: Rect) {
    use crate::commands::slash::get_commands_text;

    // Calculate modal size - take up most of the screen
    let modal_width = (area.width as f32 * 0.9) as u16;
    let modal_height = (area.height as f32 * 0.9) as u16;

    let popup_area = Rect {
        x: (area.width - modal_width) / 2,
        y: (area.height - modal_height) / 2,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Create the modal block
    let block = Block::default()
        .title(" Commands Reference (press any key to close) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_ACCENT))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Get the commands text and parse it into lines
    let commands_text = get_commands_text();
    let lines: Vec<Line> = commands_text
        .lines()
        .map(|line| {
            // Apply styling based on line content
            if line.starts_with("━") {
                // Separator lines
                Line::from(Span::styled(line, Style::default().fg(BORDER_ACCENT)))
            } else if line.starts_with("🔧")
                || line.starts_with("💬")
                || line.starts_with("🧠")
                || line.starts_with("⚙️")
                || line.starts_with("📁")
                || line.starts_with("📋")
                || line.starts_with("📎")
                || line.starts_with("🖥️")
            {
                // Section headers
                Line::from(Span::styled(
                    line,
                    Style::default()
                        .fg(ACCENT_CYAN)
                        .add_modifier(Modifier::BOLD),
                ))
            } else if line.trim().starts_with("/")
                || line.trim().starts_with("@")
                || line.trim().starts_with("!")
            {
                // Command lines
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                if parts.len() >= 2 {
                    let rest = format!(" {}", parts[1]);
                    vec![
                        Span::styled("  ", Style::default()),
                        Span::styled(
                            parts[0],
                            Style::default()
                                .fg(ACCENT_GREEN)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(rest, Style::default().fg(TEXT_SECONDARY)),
                    ]
                    .into()
                } else {
                    Line::from(Span::styled(line, Style::default().fg(ACCENT_GREEN)))
                }
            } else if line.starts_with("💡") {
                // Tips section
                Line::from(Span::styled(
                    line,
                    Style::default()
                        .fg(ACCENT_YELLOW)
                        .add_modifier(Modifier::ITALIC),
                ))
            } else if line.trim().starts_with("•") {
                // Bullet points
                Line::from(Span::styled(line, Style::default().fg(TEXT_PRIMARY)))
            } else if line.trim().is_empty() {
                // Empty lines
                Line::from("")
            } else {
                // Regular text
                Line::from(Span::styled(line, Style::default().fg(TEXT_SECONDARY)))
            }
        })
        .collect();

    // Create scrollable paragraph
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((0, 0)); // TODO: Add scrolling support with arrow keys

    f.render_widget(paragraph, inner);
}
