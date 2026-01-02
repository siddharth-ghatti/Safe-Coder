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

    // Main layout: [title] [messages] [input hints] [input] [status bar]
    let chunks = Layout::vertical([
        Constraint::Length(1),                           // Title bar
        Constraint::Min(5),                              // Messages
        Constraint::Length(1),                           // Input hints (enter send / model)
        Constraint::Length(calculate_input_height(app)), // Input area
        Constraint::Length(1),                           // Status bar
    ])
    .split(size);

    draw_title_bar(f, app, chunks[0]);
    draw_messages(f, app, chunks[1]);
    draw_input_hints(f, app, chunks[2]);
    draw_input_area(f, app, chunks[3]);
    draw_status_bar(f, app, chunks[4]);

    // Popups on top
    if app.file_picker.visible {
        draw_file_picker_popup(f, app, size);
    }

    if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
        draw_autocomplete_popup(f, app, size);
    }
}

fn calculate_input_height(app: &ShellTuiApp) -> u16 {
    let lines = app.input.lines().count().max(1).min(5) as u16;
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
            // User command header
            lines.push(MessageLine::UserHeader {
                text: block.input.clone(),
            });

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
            // User query header (like "# Change button color to danger...")
            lines.push(MessageLine::UserHeader {
                text: block.input.clone(),
            });

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

    // Build input with cursor
    let (before, after) = app.input.split_at(app.cursor_pos.min(app.input.len()));

    // Blinking cursor
    let cursor_visible = app.animation_frame % 16 < 10;
    let cursor_char = if cursor_visible { "█" } else { " " };

    let after_rest: String = if !after.is_empty() {
        after.chars().skip(1).collect()
    } else {
        String::new()
    };

    let mut spans = vec![Span::styled("> ", Style::default().fg(TEXT_DIM))];

    if app.input.is_empty() {
        // Show placeholder when empty, with blinking cursor after prompt
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default()
                .fg(ACCENT_CYAN)
                .add_modifier(Modifier::REVERSED),
        ));
        spans.push(Span::styled(
            "Type a message...",
            Style::default().fg(TEXT_MUTED),
        ));
    } else {
        spans.push(Span::styled(
            before.to_string(),
            Style::default().fg(TEXT_PRIMARY),
        ));
        spans.push(Span::styled(
            cursor_char.to_string(),
            Style::default()
                .fg(ACCENT_CYAN)
                .add_modifier(Modifier::REVERSED),
        ));
        spans.push(Span::styled(after_rest, Style::default().fg(TEXT_PRIMARY)));
    }

    let input_para = Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false });

    f.render_widget(input_para, inner);
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

    // Build LSP status string (like Crush: "● Go gopls  ● Rust rust-analyzer")
    let lsp_status: String = app
        .lsp_servers
        .iter()
        .filter(|(_, _, running)| *running)
        .map(|(lang, cmd, _)| format!("● {} {}", lang, cmd))
        .collect::<Vec<_>>()
        .join("  ");

    // Build spans
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

    // Add LSP status
    spans.push(Span::styled("  │  ", Style::default().fg(TEXT_MUTED)));
    if app.lsp_initializing {
        // Show initializing spinner
        let spinner_chars = ["◐", "◓", "◑", "◒"];
        let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
        spans.push(Span::styled(
            format!("{} LSP initializing...", spinner),
            Style::default().fg(TEXT_DIM),
        ));
    } else if !lsp_status.is_empty() {
        // Show running servers
        spans.push(Span::styled(lsp_status, Style::default().fg(ACCENT_GREEN)));
    } else if let Some(ref msg) = app.lsp_status_message {
        // Show error/status message
        let color = if msg.contains("failed") || msg.contains("error") {
            ACCENT_RED
        } else {
            TEXT_DIM
        };
        spans.push(Span::styled(msg.clone(), Style::default().fg(color)));
    }

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
