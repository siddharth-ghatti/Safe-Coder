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

use super::markdown::{has_markdown, render_markdown_lines};
use super::shell_app::{BlockOutput, BlockType, CommandBlock, FileDiff, ShellTuiApp};

use super::sidebar::{PlanStepDisplay, TodoPlanDisplay, ToolStepStatus};
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

const BG_PRIMARY: Color = Color::Rgb(0, 0, 0); // Pure black background
const BG_BLOCK: Color = Color::Rgb(15, 15, 15); // Slightly lighter for blocks
const BG_INPUT: Color = Color::Rgb(20, 20, 20); // Input area
const BG_STATUS: Color = Color::Rgb(10, 10, 10); // Status bar

const BORDER_SUBTLE: Color = Color::Rgb(40, 40, 45); // Subtle borders

// Diff colors (for file changes)
const BG_DIFF_ADD: Color = Color::Rgb(30, 60, 30); // Green background for additions
const BG_DIFF_DEL: Color = Color::Rgb(60, 30, 30); // Red background for deletions
const BORDER_ACCENT: Color = Color::Rgb(80, 200, 220); // Accent borders

// ============================================================================
// Animation Constants
// ============================================================================

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract meaningful target from tool input (Claude Code style)
/// For Read/Write/Edit: extract file path
/// For Glob/Grep: extract pattern
/// For Bash: extract command (truncated)
fn extract_tool_target(tool_name: &str, input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    match tool_name.to_lowercase().as_str() {
        "read" | "write" | "edit" => {
            // Try to extract file path from input
            // Input might be "path/to/file" or "Reading path/to/file"
            let input = input.trim();
            // If starts with common path chars, use as-is
            if input.starts_with('/') || input.starts_with('.') || input.starts_with("src") {
                return input.to_string();
            }
            // Otherwise try to find a path-like string
            for word in input.split_whitespace() {
                if word.contains('/') || word.contains('.') {
                    return word.trim_matches(|c| c == '\'' || c == '"').to_string();
                }
            }
            input.to_string()
        }
        "glob" => {
            // Extract glob pattern
            for word in input.split_whitespace() {
                if word.contains('*') || word.contains('/') {
                    return word.trim_matches(|c| c == '\'' || c == '"').to_string();
                }
            }
            input.to_string()
        }
        "grep" | "code_search" => {
            // Extract search pattern
            if let Some(pattern) = input.split('"').nth(1) {
                return format!("\"{}\"", pattern);
            }
            if let Some(pattern) = input.split('\'').nth(1) {
                return format!("'{}'", pattern);
            }
            input.to_string()
        }
        "bash" => {
            // For bash, truncate long commands
            let cmd = input.trim();
            if cmd.chars().count() > 40 {
                format!("{}...", truncate_str(cmd, 37))
            } else {
                cmd.to_string()
            }
        }
        _ => input.to_string(),
    }
}

/// Capitalize first letter of a string
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// Use shared truncate_str from utils
use crate::utils::truncate_str;

// ============================================================================
// Main Draw Function
// ============================================================================

pub fn draw(f: &mut Frame, app: &mut ShellTuiApp) {
    let size = f.area();

    // Fill background
    let bg = Block::default().style(Style::default().bg(BG_PRIMARY));
    f.render_widget(bg, size);

    // Responsive sidebar: hide on very small windows, narrow on small, normal on large
    let sidebar_width = if !app.sidebar.visible {
        0
    } else if size.width < 80 {
        0 // Auto-hide sidebar on very narrow windows
    } else if size.width < 120 {
        22 // Narrow sidebar for small windows
    } else {
        26 // Normal sidebar
    };

    let horizontal = Layout::horizontal([
        Constraint::Min(30),               // Main content area (reduced min)
        Constraint::Length(sidebar_width), // Sidebar (responsive width)
    ])
    .split(size);

    let main_area = horizontal[0];
    let sidebar_area = horizontal[1];

    // Compact layout: [messages] [input] [status bar] (no title bar, hints in status)
    let chunks = Layout::vertical([
        Constraint::Min(3),                              // Messages
        Constraint::Length(calculate_input_height(app)), // Input area
        Constraint::Length(1),                           // Status bar (includes hints)
    ])
    .split(main_area);

    draw_messages(f, app, chunks[0]);
    draw_input_area(f, app, chunks[1]);
    draw_status_bar(f, app, chunks[2]);

    // Draw sidebar if visible
    if app.sidebar.visible {
        draw_sidebar(f, app, sidebar_area);
    }

    // Popups on top
    if app.model_picker.visible {
        draw_model_picker_popup(f, app, size);
    }

    if app.file_picker.visible {
        draw_file_picker_popup(f, app, size);
    }

    // Command autocomplete popup (for slash commands)
    if app.command_autocomplete.visible && !app.command_autocomplete.suggestions.is_empty() {
        draw_command_autocomplete_popup(f, app, size);
    }

    if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
        draw_autocomplete_popup(f, app, size);
    }

    if app.commands_modal_visible {
        draw_commands_modal(f, app, size);
    }

    // Plan approval popup (highest priority)
    if app.plan_approval_visible {
        draw_plan_approval_popup(f, app, size);
    }

    // Tool approval modal (highest priority when shown)
    if app.pending_tool_approval.is_some() {
        draw_tool_approval_modal(f, app, size);
    }
}

fn calculate_input_height(app: &ShellTuiApp) -> u16 {
    // Compact input: 1-3 lines max, minimal borders
    let estimated_width = 70usize;
    let wrapped_count = if app.input.is_empty() {
        1
    } else {
        let mut count = 0;
        for line in app.input.lines() {
            count += ((line.len() / estimated_width) + 1).max(1);
        }
        if count == 0 {
            count = ((app.input.len() / estimated_width) + 1).max(1);
        }
        count
    };
    // Max 3 lines, minimal padding
    let lines = wrapped_count.min(3) as u16;
    lines + 1 // just top border
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
    let max_visible = area.height as usize;

    // Track width changes for proper reflow (width-agnostic rendering)
    if app.cached_render_width != content_width {
        app.cached_render_width = content_width;
        // Width changed - invalidate any cached line counts
        app.cached_total_lines = 0;
    }

    // Performance optimization: Only process recent blocks when at bottom (common case)
    // When scrolled up, we need to process more blocks
    let blocks_to_process = if app.scroll_offset == 0 {
        // At bottom: only process last ~10 blocks (usually enough for viewport)
        app.blocks.len().min(15)
    } else {
        // Scrolled up: process more blocks based on scroll offset
        let estimated_blocks_needed = (app.scroll_offset / 10) + 15;
        app.blocks.len().min(estimated_blocks_needed)
    };

    let start_block = app.blocks.len().saturating_sub(blocks_to_process);

    // Estimate lines for blocks we're skipping (width-aware for accurate scrollbar)
    let skipped_lines: usize = app
        .blocks
        .iter()
        .take(start_block)
        .map(|b| estimate_block_line_count_width(b, content_width))
        .sum();

    // Build rendered lines for visible portion
    let mut all_lines: Vec<MessageLine> = Vec::with_capacity(max_visible * 3);

    // Get current task text for shimmer display (prefer task over spinner word)
    let current_task = app.sidebar.current_task_active_form();
    let status_text = current_task.as_deref().unwrap_or_else(|| app.spinner.current());

    // Get todos for inline display during AI processing
    // Use inline_todos directly as the source of truth
    let todos = if !app.inline_todos.is_empty() {
        Some(&app.inline_todos)
    } else {
        None
    };

    // Compute elapsed time for status line
    let elapsed_secs = chrono::Local::now()
        .signed_duration_since(app.start_time)
        .num_seconds();
    let elapsed_str = if elapsed_secs >= 3600 {
        format!(
            "{}h {}m",
            elapsed_secs / 3600,
            (elapsed_secs % 3600) / 60
        )
    } else if elapsed_secs >= 60 {
        format!("{}m {}s", elapsed_secs / 60, elapsed_secs % 60)
    } else {
        format!("{}s", elapsed_secs)
    };

    // Get total tokens
    let total_tokens = app.sidebar.token_usage.total_tokens;

    for block in app.blocks.iter().skip(start_block) {
        render_block(
            &mut all_lines,
            block,
            content_width,
            app.animation_frame,
            &app.model_display,
            status_text,
            todos,
            &elapsed_str,
            total_tokens,
        );
        all_lines.push(MessageLine::Empty);
    }

    // Total includes skipped + rendered
    let total_lines = skipped_lines + all_lines.len();

    // Calculate visible window
    let effective_scroll = app.scroll_offset.saturating_sub(skipped_lines);
    let visible_start = if all_lines.len() > max_visible {
        all_lines
            .len()
            .saturating_sub(max_visible)
            .saturating_sub(effective_scroll)
    } else {
        0
    };
    let visible_end = (visible_start + max_visible).min(all_lines.len());

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

/// Fast line count estimate for a block (width-aware for better scrollbar accuracy)
#[inline]
fn estimate_block_line_count_width(block: &CommandBlock, width: usize) -> usize {
    // Estimate wrapped lines based on average line length vs width
    let base = match &block.output {
        BlockOutput::Streaming { lines, .. } => {
            let total_chars: usize = lines.iter().map(|l| l.len()).sum();
            let estimated_lines = if width > 0 {
                (total_chars / width.max(40)).max(lines.len())
            } else {
                lines.len()
            };
            estimated_lines.min(50)
        }
        BlockOutput::Success(text) => {
            let text_lines: Vec<&str> = text.lines().collect();
            let total_chars: usize = text_lines.iter().map(|l| l.len()).sum();
            let estimated_lines = if width > 0 {
                (total_chars / width.max(40)).max(text_lines.len())
            } else {
                text_lines.len()
            };
            estimated_lines.min(30)
        }
        BlockOutput::Error { .. } => 5,
        BlockOutput::Pending => 1,
    };
    let children: usize = block.children.iter().map(|c| estimate_block_line_count_width(c, width)).sum();
    base + children + 3 // +3 for headers/gaps
}

/// Fast line count estimate for a block (avoids full rendering)
/// Falls back to fixed-width estimate when width unknown
#[inline]
fn estimate_block_line_count(block: &CommandBlock) -> usize {
    estimate_block_line_count_width(block, 80) // Default to 80 chars
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
    // Tool header: "Edit packages/frontend/..." with optional duration
    ToolHeader {
        tool: String,
        target: String,
        duration_ms: Option<u64>,
        exit_code: Option<i32>,
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
    // Markdown-rendered line with border
    AiMarkdownLine {
        spans: Vec<Span<'static>>,
    },
    // AI thinking/reasoning text (dimmed, with thinking prefix)
    ThinkingText {
        text: String,
    },
    // AI reasoning explanation (more prominent, shown before tool calls)
    ReasoningText {
        text: String,
    },
    // Tool metadata footer (exit code, duration, truncation info)
    ToolFooter {
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        lines_shown: usize,
        lines_total: usize,
        truncated: bool,
    },
    // Tool summary line: "└ Read 50 lines" or "└ Added 6 lines, removed 17 lines"
    ToolSummary {
        text: String,
    },
    // Tool output preview line (indented)
    ToolPreviewLine {
        text: String,
    },
    // Truncation hint: "... +8 lines (ctrl+o to expand)"
    ToolTruncated {
        hidden_count: usize,
    },
    // Diagnostic counts after file operations
    DiagnosticInfo {
        errors: usize,
        warnings: usize,
    },
    // Inline todo status line (like Claude Code)
    TodoStatusLine {
        current_task: String,
        elapsed: String,
        tokens: usize,
    },
    // Inline todo item
    TodoItem {
        text: String,
        status: String, // "completed", "in_progress", "pending"
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

            MessageLine::BlockStart => Line::from(""),  // Empty line as separator (cleaner look)

            MessageLine::AiText {
                text,
                model,
                timestamp,
            } => {
                // Cleaner AI text without border
                let mut spans = vec![
                    Span::styled(text.clone(), Style::default().fg(TEXT_PRIMARY)),
                ];

                // Add model/timestamp on first line if present
                if let (Some(m), Some(t)) = (model, timestamp) {
                    spans.push(Span::styled(
                        format!("  {} ({})", m, t),
                        Style::default().fg(TEXT_DIM),
                    ));
                }

                Line::from(spans)
            }

            MessageLine::ToolHeader { tool, target, duration_ms, exit_code } => {
                // Claude Code style: "● Tool(target)" - compact, no emojis
                let status_color = match exit_code {
                    Some(0) => ACCENT_GREEN,
                    Some(_) => ACCENT_RED,
                    None => ACCENT_CYAN, // Running
                };

                let mut spans = vec![
                    Span::styled("● ", Style::default().fg(status_color)),
                    Span::styled(
                        tool.clone(),
                        Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD),
                    ),
                ];

                // Add target in parentheses if not empty (truncate long targets)
                if !target.is_empty() {
                    let display_target = if target.chars().count() > 50 {
                        format!("{}...", truncate_str(&target, 47))
                    } else {
                        target.clone()
                    };
                    spans.push(Span::styled("(", Style::default().fg(TEXT_DIM)));
                    spans.push(Span::styled(display_target, Style::default().fg(TEXT_SECONDARY)));
                    spans.push(Span::styled(")", Style::default().fg(TEXT_DIM)));
                }

                // Add duration if complete
                if let Some(ms) = duration_ms {
                    spans.push(Span::styled(
                        format!(" {}",  format_duration(*ms)),
                        Style::default().fg(TEXT_MUTED),
                    ));
                }

                Line::from(spans)
            }

            MessageLine::DiffContext {
                old_num,
                new_num,
                text,
            } => Line::from(vec![
                Span::styled("  ", Style::default()),
                Span::styled(format!("{:>3}", old_num), Style::default().fg(TEXT_DIM)),
                Span::styled(format!("{:>4}", new_num), Style::default().fg(TEXT_DIM)),
                Span::styled("   ", Style::default()),
                Span::styled(text.clone(), Style::default().fg(TEXT_SECONDARY)),
            ]),

            MessageLine::DiffRemove { old_num, text } => Line::from(vec![
                Span::styled(
                    format!("{:>3}-", old_num),
                    Style::default().fg(TEXT_MUTED).bg(BG_DIFF_DEL),
                ),
                Span::styled(
                    text.clone(),
                    Style::default().fg(Color::Rgb(255, 180, 180)).bg(BG_DIFF_DEL),
                ),
            ]),

            MessageLine::DiffAdd { new_num, text } => Line::from(vec![
                Span::styled(
                    format!("{:>3}+", new_num),
                    Style::default().fg(TEXT_MUTED).bg(BG_DIFF_ADD),
                ),
                Span::styled(
                    text.clone(),
                    Style::default().fg(Color::Rgb(180, 255, 180)).bg(BG_DIFF_ADD),
                ),
            ]),

            MessageLine::ShellOutput { text } => Line::from(vec![
                Span::styled("  ", Style::default()),  // Indent
                Span::styled(text.clone(), Style::default().fg(TEXT_DIM)),
            ]),

            MessageLine::SystemInfo { text } => {
                Line::from(Span::styled(text.clone(), Style::default().fg(TEXT_DIM)))
            }

            MessageLine::Running {
                text,
                spinner_frame,
            } => {
                // Claude Code style: "* Running..."
                let spinners = ["*", "○", "◌", "●"];
                let spinner = spinners[*spinner_frame / 5 % spinners.len()];
                Line::from(vec![
                    Span::styled(format!("{} ", spinner), Style::default().fg(ACCENT_CYAN)),
                    Span::styled(text.clone(), Style::default().fg(TEXT_DIM)),
                ])
            }

            MessageLine::AiMarkdownLine { spans } => {
                // Cleaner markdown without border
                Line::from(spans.clone())
            }

            MessageLine::ThinkingText { text } => {
                // Cleaner thinking/reasoning with dimmed style
                Line::from(vec![
                    Span::styled(
                        text.clone(),
                        Style::default()
                            .fg(TEXT_DIM)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ])
            }

            MessageLine::ReasoningText { text } => {
                // Prominent reasoning with cyan prefix
                Line::from(vec![
                    Span::styled("→ ", Style::default().fg(ACCENT_CYAN)),
                    Span::styled(
                        text.clone(),
                        Style::default().fg(TEXT_SECONDARY),
                    ),
                ])
            }

            MessageLine::ToolFooter { exit_code, duration_ms: _, lines_shown, lines_total, truncated } => {
                // Compact footer: "└ X lines" or "└ exit N"
                let mut spans = vec![Span::styled("  └ ", Style::default().fg(TEXT_MUTED))];

                // Show error exit code if non-zero
                if let Some(code) = exit_code {
                    if *code != 0 {
                        spans.push(Span::styled(
                            format!("exit {} ", code),
                            Style::default().fg(ACCENT_RED),
                        ));
                    }
                }

                // Line count info (compact)
                if *truncated {
                    spans.push(Span::styled(
                        format!("{}/{} lines", lines_shown, lines_total),
                        Style::default().fg(TEXT_MUTED),
                    ));
                } else if *lines_total > 0 {
                    spans.push(Span::styled(
                        format!("{} lines", lines_total),
                        Style::default().fg(TEXT_MUTED),
                    ));
                }

                Line::from(spans)
            }

            MessageLine::ToolSummary { text } => {
                // Claude Code style: "  └ Summary text"
                Line::from(vec![
                    Span::styled("  └ ", Style::default().fg(TEXT_MUTED)),
                    Span::styled(text.clone(), Style::default().fg(TEXT_SECONDARY)),
                ])
            }

            MessageLine::ToolPreviewLine { text } => {
                // Indented preview line
                Line::from(vec![
                    Span::styled("    ", Style::default()),
                    Span::styled(text.clone(), Style::default().fg(TEXT_MUTED)),
                ])
            }

            MessageLine::ToolTruncated { hidden_count } => {
                // "  ... +N lines (ctrl+o to expand)"
                Line::from(vec![
                    Span::styled(
                        format!("  ... +{} lines (ctrl+o to expand)", hidden_count),
                        Style::default().fg(TEXT_DIM),
                    ),
                ])
            }

            MessageLine::DiagnosticInfo { errors, warnings } => {
                let mut spans = vec![Span::styled("  ", Style::default())];

                // Only show if there are diagnostics
                if *errors > 0 || *warnings > 0 {
                    if *errors > 0 {
                        spans.push(Span::styled(
                            format!("● {} error{}", errors, if *errors == 1 { "" } else { "s" }),
                            Style::default().fg(ACCENT_RED),
                        ));
                    }
                    if *warnings > 0 {
                        if *errors > 0 {
                            spans.push(Span::styled(", ", Style::default().fg(TEXT_DIM)));
                        }
                        spans.push(Span::styled(
                            format!("▲ {} warning{}", warnings, if *warnings == 1 { "" } else { "s" }),
                            Style::default().fg(ACCENT_YELLOW),
                        ));
                    }
                }

                Line::from(spans)
            }

            MessageLine::TodoStatusLine { current_task, elapsed, tokens } => {
                // Format like: "· Adding feature... (ctrl+c to interrupt · 6m 17s · ↑ 15.3k tokens)"
                let tokens_str = if *tokens >= 1_000_000 {
                    format!("{:.1}M", *tokens as f64 / 1_000_000.0)
                } else if *tokens >= 1_000 {
                    format!("{:.1}k", *tokens as f64 / 1_000.0)
                } else {
                    tokens.to_string()
                };

                Line::from(vec![
                    Span::styled("· ", Style::default().fg(ACCENT_CYAN)),
                    Span::styled(
                        format!("{}  ", current_task),
                        Style::default().fg(TEXT_PRIMARY),
                    ),
                    Span::styled(
                        format!("(ctrl+c to interrupt · {} · ↑ {} tokens)", elapsed, tokens_str),
                        Style::default().fg(TEXT_DIM),
                    ),
                ])
            }

            MessageLine::TodoItem { text, status } => {
                // Claude Code style: bullet point with status suffix
                let (prefix, text_style, suffix) = match status.as_str() {
                    "completed" => (
                        "  [x] ",
                        Style::default().fg(TEXT_DIM),
                        " - completed",
                    ),
                    "in_progress" => (
                        "  [>] ",
                        Style::default().fg(TEXT_PRIMARY),
                        "",
                    ),
                    _ => (
                        "  [ ] ",
                        Style::default().fg(TEXT_SECONDARY),
                        "",
                    ),
                };

                Line::from(vec![
                    Span::styled(prefix, text_style),
                    Span::styled(text.clone(), text_style),
                    Span::styled(suffix, Style::default().fg(TEXT_DIM)),
                ])
            }
        }
    }
}

// ============================================================================
// Block Rendering
// ============================================================================

fn render_block(
    lines: &mut Vec<MessageLine>,
    block: &CommandBlock,
    width: usize,
    frame: usize,
    model_display: &str,
    spinner_word: &str,
    todos: Option<&Vec<crate::tools::todo::TodoItem>>,
    _elapsed_str: &str,
    _total_tokens: usize,
) {
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

            // Show status while running - use rotating cool words
            let is_running = block.is_running();
            let has_children = !block.children.is_empty();
            let has_output = !block.output.get_text().is_empty();

            // Initial thinking state - show Running message with animation
            if is_running && !has_children && !has_output {
                let status = if spinner_word.contains(' ') {
                    spinner_word.to_string()
                } else {
                    format!("{}...", spinner_word)
                };
                lines.push(MessageLine::Running {
                    text: status,
                    spinner_frame: frame,
                });
            }

            // Render children (tools, reasoning)
            let last_running_idx = block.children.iter().rposition(|c| c.is_running());
            for (i, child) in block.children.iter().enumerate() {
                let show_spinner = last_running_idx == Some(i);
                render_child_block(lines, child, width, frame, false, show_spinner);
            }

            // Show status after tools complete but still processing
            if is_running && has_children && !has_output {
                let all_children_done = block.children.iter().all(|c| !c.is_running());
                if all_children_done {
                    let status = if spinner_word.contains(' ') {
                        spinner_word.to_string()
                    } else {
                        format!("{}...", spinner_word)
                    };
                    lines.push(MessageLine::Running {
                        text: status,
                        spinner_frame: frame,
                    });
                }
            }

            // Inline todo list display - shows persistently below the animated status
            if let Some(todo_items) = todos {
                if !todo_items.is_empty() {
                    // Todo items with their status
                    for item in todo_items {
                        lines.push(MessageLine::TodoItem {
                            text: if item.status == "in_progress" && !item.active_form.is_empty() {
                                item.active_form.clone()
                            } else {
                                item.content.clone()
                            },
                            status: item.status.clone(),
                        });
                    }
                }
            }

            // Final AI response - render with markdown if detected
            let text = block.output.get_text();
            if !text.is_empty() {
                lines.push(MessageLine::Empty);
                lines.push(MessageLine::BlockStart);

                if has_markdown(&text) {
                    // Use full markdown rendering for complex content
                    let md_lines = render_markdown_lines(&text);
                    let total = md_lines.len();

                    for (i, md_line) in md_lines.into_iter().enumerate() {
                        let is_last = i == total - 1;
                        if is_last {
                            // Add model/timestamp info on last line
                            let mut spans: Vec<Span<'static>> = md_line.spans;
                            spans.push(Span::styled(
                                format!(
                                    "  {} ({})",
                                    shorten_model_name(model_display),
                                    chrono::Local::now().format("%I:%M %p")
                                ),
                                Style::default().fg(TEXT_DIM),
                            ));
                            lines.push(MessageLine::AiMarkdownLine { spans });
                        } else {
                            lines.push(MessageLine::AiMarkdownLine {
                                spans: md_line.spans,
                            });
                        }
                    }
                } else {
                    // Plain text fallback for simple content
                    let output_lines: Vec<&str> = text.lines().collect();
                    let total = output_lines.len();

                    for (i, line) in output_lines.iter().enumerate() {
                        for wrapped in wrap(line, width.saturating_sub(4)) {
                            let is_last = i == total - 1;
                            lines.push(MessageLine::AiText {
                                text: wrapped.to_string(),
                                model: if is_last {
                                    Some(shorten_model_name(model_display))
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
        }

        BlockType::Orchestration => {
            lines.push(MessageLine::UserHeader {
                text: format!("orchestrate: {}", block.input),
            });

            // Use unthrottled output for orchestration - show all output continuously
            render_output_unthrottled(lines, &block.output, width);

            // Find the last running child to only show spinner for that one
            let last_running_idx = block.children.iter().rposition(|c| c.is_running());
            for (i, child) in block.children.iter().enumerate() {
                let show_spinner = last_running_idx == Some(i);
                // Pass unthrottled=true for orchestration child blocks
                render_child_block(lines, child, width, frame, true, show_spinner);
            }
        }

        BlockType::AiToolExecution { .. } => {
            // For standalone tool execution blocks, always show spinner if running
            render_child_block(lines, block, width, frame, false, block.is_running());
        }

        BlockType::AiReasoning => {
            // Render reasoning with prominent cyan arrow prefix
            let text = block.output.get_text();
            if !text.is_empty() {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    for wrapped in wrap(trimmed, width.saturating_sub(4)) {
                        lines.push(MessageLine::ReasoningText {
                            text: wrapped.to_string(),
                        });
                    }
                }
            }
        }

        BlockType::AiThinking => {
            // Render thinking/reasoning BEFORE tool calls with a distinct style
            let text = block.output.get_text();
            if !text.is_empty() {
                lines.push(MessageLine::Empty);
                // Render thinking text in a dimmed, italicized style
                for line in text.lines() {
                    // Skip very long lines that are just whitespace or code dumps
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    for wrapped in wrap(line, width.saturating_sub(6)) {
                        lines.push(MessageLine::ThinkingText {
                            text: wrapped.to_string(),
                        });
                    }
                }
            }
        }

        BlockType::Subagent { kind } => {
            // Render subagent like an AI tool execution
            lines.push(MessageLine::UserHeader {
                text: format!("Subagent: {} - {}", kind, block.input),
            });

            render_output(lines, &block.output, width);

            // Find the last running child to only show spinner for that one
            let last_running_idx = block.children.iter().rposition(|c| c.is_running());
            for (i, child) in block.children.iter().enumerate() {
                let show_spinner = last_running_idx == Some(i);
                render_child_block(lines, child, width, frame, false, show_spinner);
            }
        }
    }
}

fn render_child_block(
    lines: &mut Vec<MessageLine>,
    block: &CommandBlock,
    width: usize,
    frame: usize,
    unthrottled: bool,
    show_spinner: bool, // Only show spinner for the last running block
) {
    match &block.block_type {
        BlockType::AiToolExecution { tool_name } => {
            // Show reasoning BEFORE the tool (if any)
            if let Some(reasoning) = &block.reasoning {
                for line in reasoning.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() {
                        for wrapped in wrap(trimmed, width.saturating_sub(4)) {
                            lines.push(MessageLine::ReasoningText {
                                text: wrapped.to_string(),
                            });
                        }
                    }
                }
            }

            // Tool header - extract meaningful target from input
            let target = extract_tool_target(tool_name, &block.input);

            // Capitalize tool name
            let tool_display = match tool_name.as_str() {
                "bash" => "Bash".to_string(),
                "read" | "Read" => "Read".to_string(),
                "write" | "Write" => "Write".to_string(),
                "edit" | "Edit" => "Edit".to_string(),
                "glob" | "Glob" => "Glob".to_string(),
                "grep" | "Grep" => "Search".to_string(),  // Use "Search" like Claude Code
                "code_search" => "Search".to_string(),
                name if name.starts_with("task-") => "Task".to_string(),
                other => capitalize_first(other),
            };

            lines.push(MessageLine::ToolHeader {
                tool: tool_display,
                target,
                duration_ms: block.duration_ms,
                exit_code: block.exit_code,
            });

            // Show spinner while running
            if block.is_running() && show_spinner {
                lines.push(MessageLine::Running {
                    text: "...".to_string(),
                    spinner_frame: frame,
                });
            } else if !block.is_running() {
                // Claude Code style: summary line first, then preview
                // Show diff summary for edit_file/write_file
                if let Some(diff) = &block.diff {
                    // Count line changes
                    let old_lines = diff.old_content.lines().count();
                    let new_lines = diff.new_content.lines().count();
                    let added = new_lines.saturating_sub(old_lines);
                    let removed = old_lines.saturating_sub(new_lines);

                    let summary = if added > 0 && removed > 0 {
                        format!("Added {} lines, removed {} lines", added, removed)
                    } else if added > 0 {
                        format!("Added {} lines", added)
                    } else if removed > 0 {
                        format!("Removed {} lines", removed)
                    } else {
                        format!("Modified {} lines", new_lines.min(old_lines).max(1))
                    };
                    lines.push(MessageLine::ToolSummary { text: summary });

                    // Show preview of new content (limited lines)
                    let preview_lines: Vec<&str> = diff.new_content.lines().take(4).collect();
                    for (i, line) in preview_lines.iter().enumerate() {
                        let line_num = new_lines.saturating_sub(preview_lines.len()) + i + 1;
                        lines.push(MessageLine::ToolPreviewLine {
                            text: format!("{:>4}  {}", line_num, line),
                        });
                    }
                    if new_lines > 4 && !unthrottled {
                        lines.push(MessageLine::ToolTruncated {
                            hidden_count: new_lines - 4,
                        });
                    }
                } else {
                    // For non-diff tools, generate smart summary and preview
                    render_tool_output_claude_style(lines, tool_name, &block.output, width, unthrottled, block.exit_code);
                }
            }

            // Show diagnostic counts if present
            if let Some((errors, warnings)) = block.diagnostic_counts {
                if errors > 0 || warnings > 0 {
                    lines.push(MessageLine::DiagnosticInfo { errors, warnings });
                }
            }
        }

        BlockType::AiReasoning => {
            let text = block.output.get_text();
            if !text.is_empty() {
                lines.push(MessageLine::Empty);
                if has_markdown(&text) {
                    // Render with markdown formatting
                    for md_line in render_markdown_lines(&text) {
                        lines.push(MessageLine::AiMarkdownLine {
                            spans: md_line.spans,
                        });
                    }
                } else {
                    // Plain text fallback
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
        }

        BlockType::AiThinking => {
            // Render thinking/reasoning with distinct dimmed style
            let text = block.output.get_text();
            if !text.is_empty() {
                for line in text.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    for wrapped in wrap(line, width.saturating_sub(6)) {
                        lines.push(MessageLine::ThinkingText {
                            text: wrapped.to_string(),
                        });
                    }
                }
            }
        }

        _ => {}
    }
}

fn render_output(lines: &mut Vec<MessageLine>, output: &BlockOutput, width: usize) {
    render_output_with_limit(lines, output, width, Some(20))
}

/// Render output without any line limit (for orchestration, continuous streaming)
fn render_output_unthrottled(lines: &mut Vec<MessageLine>, output: &BlockOutput, width: usize) {
    render_output_with_limit(lines, output, width, None)
}

/// Render output with configurable line limit (None = unlimited)
fn render_output_with_limit(
    lines: &mut Vec<MessageLine>,
    output: &BlockOutput,
    width: usize,
    max_lines: Option<usize>,
) {
    match output {
        BlockOutput::Streaming {
            lines: output_lines,
            ..
        } => {
            // For unlimited (None), show all lines; for limited, show last N lines
            let skip_count = match max_lines {
                Some(limit) if output_lines.len() > limit => output_lines.len() - limit,
                _ => 0,
            };

            for line in output_lines.iter().skip(skip_count) {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::ShellOutput {
                        text: wrapped.to_string(),
                    });
                }
            }
        }
        BlockOutput::Success(text) if !text.is_empty() => {
            let text_lines: Vec<&str> = text.lines().collect();
            let skip_count = match max_lines {
                Some(limit) if text_lines.len() > limit => text_lines.len() - limit,
                _ => 0,
            };

            for line in text_lines.iter().skip(skip_count) {
                for wrapped in wrap(line, width.saturating_sub(4)) {
                    lines.push(MessageLine::ShellOutput {
                        text: wrapped.to_string(),
                    });
                }
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

/// Output metadata from tool rendering
struct ToolOutputMeta {
    lines_shown: usize,
    lines_total: usize,
    truncated: bool,
}

fn render_tool_output(
    lines: &mut Vec<MessageLine>,
    output: &BlockOutput,
    width: usize,
    unthrottled: bool,
) -> ToolOutputMeta {
    match output {
        BlockOutput::Streaming {
            lines: output_lines,
            ..
        } => {
            let total = output_lines.len();
            let max_lines = if unthrottled { total } else { 50 };
            let skip_count = total.saturating_sub(max_lines);
            let shown = total - skip_count;

            if skip_count > 0 {
                lines.push(MessageLine::ShellOutput {
                    text: format!("  ... ({} lines hidden)", skip_count),
                });
            }

            for line in output_lines.iter().skip(skip_count) {
                for wrapped in wrap(line, width.saturating_sub(14)) {
                    lines.push(MessageLine::ShellOutput {
                        text: format!("  {}", wrapped),
                    });
                }
            }

            ToolOutputMeta {
                lines_shown: shown,
                lines_total: total,
                truncated: skip_count > 0,
            }
        }
        BlockOutput::Success(text) if !text.is_empty() => {
            let text_lines: Vec<&str> = text.lines().collect();
            let total = text_lines.len();
            let max_lines = if unthrottled { total } else { 30 };
            let skip_count = total.saturating_sub(max_lines);
            let shown = total - skip_count;

            if skip_count > 0 {
                lines.push(MessageLine::ShellOutput {
                    text: format!("  ... ({} lines hidden)", skip_count),
                });
            }

            for line in text_lines.iter().skip(skip_count) {
                for wrapped in wrap(line, width.saturating_sub(14)) {
                    lines.push(MessageLine::ShellOutput {
                        text: format!("  {}", wrapped),
                    });
                }
            }

            ToolOutputMeta {
                lines_shown: shown,
                lines_total: total,
                truncated: skip_count > 0,
            }
        }
        _ => ToolOutputMeta {
            lines_shown: 0,
            lines_total: 0,
            truncated: false,
        },
    }
}

/// Render tool output in Claude Code style (compact summary + preview)
fn render_tool_output_claude_style(
    lines: &mut Vec<MessageLine>,
    tool_name: &str,
    output: &BlockOutput,
    _width: usize,
    unthrottled: bool,
    exit_code: Option<i32>,
) {
    let text = output.get_text();
    if text.is_empty() {
        return;
    }

    let output_lines: Vec<&str> = text.lines().collect();
    let total_lines = output_lines.len();
    let preview_count = if unthrottled { total_lines } else { 4 };

    // Generate smart summary based on tool type
    let summary = match tool_name.to_lowercase().as_str() {
        "read" | "read_file" => {
            format!("Read {} lines", total_lines)
        }
        "bash" => {
            if let Some(code) = exit_code {
                if code != 0 {
                    format!("Exit code {} ({} lines)", code, total_lines)
                } else if total_lines == 0 {
                    "Completed".to_string()
                } else {
                    format!("{} lines", total_lines)
                }
            } else if total_lines == 0 {
                "Completed".to_string()
            } else {
                format!("{} lines", total_lines)
            }
        }
        "grep" | "glob" | "code_search" | "list_file" => {
            // Count matches (lines with content)
            let matches = output_lines.iter().filter(|l| !l.trim().is_empty()).count();
            if matches == 0 {
                "No matches".to_string()
            } else {
                format!("{} matches", matches)
            }
        }
        "todowrite" | "todoread" => {
            "Updated".to_string()
        }
        _ => {
            if total_lines == 0 {
                "Done".to_string()
            } else {
                format!("{} lines", total_lines)
            }
        }
    };

    lines.push(MessageLine::ToolSummary { text: summary });

    // Show preview lines (first few lines)
    for (i, line) in output_lines.iter().take(preview_count).enumerate() {
        let line_num = i + 1;
        let preview_text = if line.chars().count() > 80 {
            format!("{:>4}  {}...", line_num, truncate_str(line, 77))
        } else {
            format!("{:>4}  {}", line_num, line)
        };
        lines.push(MessageLine::ToolPreviewLine { text: preview_text });
    }

    // Show truncation hint if there's more
    if total_lines > preview_count && !unthrottled {
        lines.push(MessageLine::ToolTruncated {
            hidden_count: total_lines - preview_count,
        });
    }
}

/// Render diff in Claude Code style with colored backgrounds
fn render_diff_opencode(lines: &mut Vec<MessageLine>, diff: &FileDiff, _width: usize) {
    let text_diff = TextDiff::from_lines(&diff.old_content, &diff.new_content);

    // Count additions and deletions for summary
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for change in text_diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions += 1,
            ChangeTag::Delete => deletions += 1,
            _ => {}
        }
    }

    // Add summary line like Claude Code: "└ Added X lines, removed Y lines"
    let summary = if additions > 0 && deletions > 0 {
        format!("└ Added {} lines, removed {} lines", additions, deletions)
    } else if additions > 0 {
        format!("└ Added {} line{}", additions, if additions == 1 { "" } else { "s" })
    } else if deletions > 0 {
        format!("└ Removed {} line{}", deletions, if deletions == 1 { "" } else { "s" })
    } else {
        "└ No changes".to_string()
    };
    lines.push(MessageLine::ShellOutput { text: summary });

    let mut old_line = 1usize;
    let mut new_line = 1usize;
    let mut changes_shown = 0;
    let max_changes = 20;

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

/// Shorten model name for display (e.g., "claude-sonnet-4-20250514" -> "claude-sonnet-4")
fn shorten_model_name(model: &str) -> String {
    // Handle common patterns
    if model.contains("claude") {
        // claude-sonnet-4-20250514 -> claude-sonnet-4
        // claude-3-5-sonnet-20241022 -> claude-3.5-sonnet
        if let Some(pos) = model.rfind("-20") {
            return model[..pos].to_string();
        }
    }
    if model.contains("/") {
        // anthropic/claude-3.5-sonnet -> claude-3.5-sonnet
        if let Some(pos) = model.rfind('/') {
            return model[pos + 1..].to_string();
        }
    }
    // Default: just return as-is, but limit length
    if model.chars().count() > 25 {
        format!("{}...", truncate_str(model, 22))
    } else {
        model.to_string()
    }
}

// ============================================================================
// Input Area
// ============================================================================

fn draw_input_area(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Compact: just top border line, no side borders
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER_SUBTLE))
        .style(Style::default().bg(BG_INPUT));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Check for attached images and adjust available height
    let has_images = app.has_attached_images();

    // Split inner area if we have images
    let (image_area, input_inner) = if has_images && inner.height > 1 {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(1),
                ratatui::layout::Constraint::Min(1),
            ])
            .split(inner);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, inner)
    };

    // Render image indicator if present
    if let Some(img_area) = image_area {
        let count = app.attached_images.len();
        let size = app.attached_images_size_display();
        let image_spans = vec![
            Span::styled("📎 ", Style::default().fg(ACCENT_YELLOW)),
            Span::styled(
                format!("{} image{} attached", count, if count == 1 { "" } else { "s" }),
                Style::default().fg(ACCENT_YELLOW),
            ),
            Span::styled(
                format!(" ({})", size),
                Style::default().fg(TEXT_DIM),
            ),
            Span::styled(
                " - Press Ctrl+Shift+V to clear",
                Style::default().fg(TEXT_MUTED),
            ),
        ];
        let image_line = Paragraph::new(Line::from(image_spans));
        f.render_widget(image_line, img_area);
    }

    let available_width = input_inner.width.saturating_sub(3) as usize; // Account for "> " prefix
    let available_height = input_inner.height as usize;

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
        f.render_widget(para, input_inner);
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
    f.render_widget(para, input_inner);
}

// ============================================================================
// Status Bar (bottom)
// ============================================================================

fn draw_status_bar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mode = app.agent_mode.short_name();
    let is_narrow = area.width < 100;

    // Compact left side
    let mut spans = vec![Span::styled(
        " safe-coder",
        Style::default()
            .fg(TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )];

    // Only show path on wider screens
    if !is_narrow {
        let path = app
            .cwd
            .to_string_lossy()
            .replace(&std::env::var("HOME").unwrap_or_default(), "~");
        let truncated_path = if path.len() > 30 {
            format!("...{}", &path[path.len()-27..])
        } else {
            path
        };
        spans.push(Span::styled(
            format!(" {}", truncated_path),
            Style::default().fg(TEXT_DIM),
        ));
    }

    // Right side: compact hints + mode
    let mut right_spans: Vec<Span> = Vec::new();

    // Show "enter to send" hint when input has content
    if !app.input.is_empty() {
        right_spans.push(Span::styled("⏎send ", Style::default().fg(TEXT_DIM)));
    }

    // Compact shortcuts
    if !is_narrow {
        right_spans.push(Span::styled("^C", Style::default().fg(TEXT_MUTED)));
        right_spans.push(Span::styled("quit ", Style::default().fg(TEXT_DIM)));
        right_spans.push(Span::styled("tab", Style::default().fg(TEXT_MUTED)));
        right_spans.push(Span::styled("mode ", Style::default().fg(TEXT_DIM)));
    }

    // Mode indicator (always visible)
    let mode_color = match mode {
        "BUILD" => ACCENT_GREEN,
        "PLAN" => ACCENT_CYAN,
        _ => TEXT_PRIMARY,
    };
    right_spans.push(Span::styled(
        mode,
        Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
    ));
    right_spans.push(Span::styled(" ", Style::default()));

    // Calculate padding
    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let right_len: usize = right_spans.iter().map(|s| s.content.len()).sum();
    let padding = area
        .width
        .saturating_sub(left_len as u16 + right_len as u16) as usize;

    spans.push(Span::styled(" ".repeat(padding.max(1)), Style::default()));
    spans.extend(right_spans);

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(BG_STATUS));
    f.render_widget(para, area);
}

// ============================================================================
// Sidebar (OpenCode-style)
// ============================================================================

fn draw_sidebar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Add left border for visual separation
    let sidebar_block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(BORDER_SUBTLE))
        .style(Style::default().bg(BG_PRIMARY));

    let inner = sidebar_block.inner(area);
    f.render_widget(sidebar_block, area);

    // Compact sidebar sections - simplified to remove the FILES/modified section
    let sections = Layout::vertical([
        Constraint::Length(2),               // MODE (compact)
        Constraint::Length(2),               // CONTEXT (compact)
        Constraint::Min(4),                  // PLAN (flexible)
        Constraint::Length(3),               // LSP (compact)
    ])
    .split(inner);

    draw_sidebar_mode(f, app, sections[0]);
    draw_sidebar_context(f, app, sections[1]);
    draw_sidebar_plan(f, app, sections[2]);
    draw_sidebar_lsp(f, app, sections[3]);
}

fn draw_sidebar_mode(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mode = app.agent_mode.short_name();
    let is_processing = app.ai_thinking;

    let mode_color = match mode {
        "BUILD" => ACCENT_GREEN,
        "PLAN" => ACCENT_CYAN,
        _ => TEXT_PRIMARY,
    };

    // Animate the dot when processing
    let dot = if is_processing {
        let frame = app.animation_frame / 3 % 4;
        match frame {
            0 => "●",
            1 => "◐",
            2 => "○",
            _ => "◑",
        }
    } else if mode == "BUILD" {
        "●"
    } else {
        "○"
    };

    let line = Line::from(vec![
        Span::styled(
            mode,
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", dot),
            Style::default().fg(if is_processing { ACCENT_CYAN } else { mode_color }),
        ),
    ]);

    let para = Paragraph::new(line);
    f.render_widget(para, area);
}

fn draw_sidebar_context(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let usage = &app.sidebar.token_usage;
    let is_processing = app.ai_thinking;

    // Compact format: "In: 3.0M / Out: 8.5K"
    let display = usage.format_display();

    // Use accent color when actively streaming tokens
    let color = if is_processing {
        ACCENT_CYAN
    } else {
        TEXT_SECONDARY
    };

    let line = Line::from(vec![
        Span::styled(display, Style::default().fg(color)),
    ]);

    let para = Paragraph::new(line);
    f.render_widget(para, area);
}

/// Format duration in human-readable form (ms, s, m)
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m{}s", mins, secs)
    }
}


fn draw_sidebar_plan(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Check if we're in build mode
    let show_tool_steps = app.agent_mode == crate::tools::AgentMode::Build;
    let is_thinking = app.ai_thinking;

    // 1. ACTIVE PLAN (High Priority)
    // If there is an active plan (agent executing), show it.
    // In Build Mode, we prioritize this over the combined view if it exists.
    if show_tool_steps && app.sidebar.active_plan.is_some() {
        draw_active_plan(f, app, area);
        return;
    }

    // 2. COMBINED VIEW (Build Mode)
    // Show Todo List AND Tool Steps if both exist
    if show_tool_steps {
        let has_todos = app
            .sidebar
            .todo_plan
            .as_ref()
            .map(|p| !p.items.is_empty())
            .unwrap_or(false);
        let has_steps = !app.sidebar.tool_steps.is_empty();

        if has_todos && has_steps {
            // Split area: 50% for Todos (Top), 50% for Steps (Bottom)
            let chunks = Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            draw_todo_list_section(f, app, chunks[0], " TASKS");
            draw_tool_steps_section(f, app, chunks[1], " STEPS");
        } else if has_todos {
            draw_todo_list_section(f, app, area, " TASKS");
        } else if has_steps {
            draw_tool_steps_section(f, app, area, " STEPS");
        } else {
            // Empty state
            draw_empty_state(f, " TASKS & STEPS", area);
        }
        return;
    }

    // 3. PLAN MODE (Standard View)
    // Show Todo List (Plan)
    if let Some(ref _todo_plan) = app.sidebar.todo_plan {
        draw_todo_list_section(f, app, area, " PLAN");
    } else if is_thinking {
        draw_thinking(f, app, area, " PLAN");
    } else {
        draw_empty_state(f, " PLAN", area);
    }
}

fn draw_active_plan(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let mut lines = vec![Line::from(Span::styled(
        " PLAN",
        Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
    ))];

    let active_plan = app.sidebar.active_plan.as_ref().unwrap();

    // Progress bar
    let percent = active_plan.progress_percent();
    let bar_width = area.width.saturating_sub(4) as usize;
    let filled = ((percent / 100.0) * bar_width as f32) as usize;
    let empty = bar_width.saturating_sub(filled);

    lines.push(Line::from(vec![
        Span::styled(" ", Style::default()),
        Span::styled("█".repeat(filled), Style::default().fg(ACCENT_GREEN)),
        Span::styled("░".repeat(empty), Style::default().fg(TEXT_MUTED)),
    ]));

    // Step count
    lines.push(Line::from(Span::styled(
        format!(
            " {}/{} steps",
            active_plan.completed_count(),
            active_plan.steps.len()
        ),
        Style::default().fg(TEXT_SECONDARY),
    )));

    if active_plan.awaiting_approval {
        lines.push(Line::from(Span::styled(
            " ⏸ Awaiting approval",
            Style::default().fg(ACCENT_YELLOW),
        )));
    }

    // Visible steps logic
    let max_items = area.height.saturating_sub(5) as usize;
    let total_steps = active_plan.steps.len();
    let in_progress_idx = active_plan.current_step_idx;

    let scroll_start = if let Some(idx) = in_progress_idx {
        if total_steps <= max_items {
            0
        } else if idx < max_items / 2 {
            0
        } else if idx > total_steps - max_items / 2 {
            total_steps.saturating_sub(max_items)
        } else {
            idx.saturating_sub(max_items / 2)
        }
    } else {
        0
    };

    let visible_steps: Vec<&PlanStepDisplay> = active_plan
        .steps
        .iter()
        .skip(scroll_start)
        .take(max_items)
        .collect();

    if scroll_start > 0 {
        lines.push(Line::from(Span::styled(
            format!(" ↑ {} more above", scroll_start),
            Style::default().fg(TEXT_MUTED),
        )));
    }

    for step in visible_steps.iter() {
        let (icon, icon_color) = match step.status {
            PlanStepStatus::Completed => ("✓".to_string(), ACCENT_GREEN),
            PlanStepStatus::InProgress => {
                let spinner_chars = ["◐", "◓", "◑", "◒"];
                let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
                (spinner.to_string(), ACCENT_CYAN)
            }
            PlanStepStatus::Failed => ("✗".to_string(), ACCENT_RED),
            PlanStepStatus::Skipped => ("⊘".to_string(), TEXT_DIM),
            PlanStepStatus::Pending => ("◯".to_string(), TEXT_DIM),
        };

        let max_len = area.width.saturating_sub(5) as usize;
        let desc = if step.description.len() > max_len {
            format!("{}...", &step.description[..max_len.saturating_sub(3)])
        } else {
            step.description.clone()
        };

        let desc_style = match step.status {
            PlanStepStatus::InProgress => Style::default().fg(TEXT_PRIMARY),
            PlanStepStatus::Completed => Style::default().fg(TEXT_DIM),
            PlanStepStatus::Failed => Style::default().fg(ACCENT_RED),
            _ => Style::default().fg(TEXT_SECONDARY),
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
            Span::styled(desc, desc_style),
        ]));
    }

    // Bottom scroll
    let items_below = total_steps.saturating_sub(scroll_start + visible_steps.len());
    if items_below > 0 {
        lines.push(Line::from(Span::styled(
            format!(" ↓ {} more below", items_below),
            Style::default().fg(TEXT_MUTED),
        )));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_todo_list_section(f: &mut Frame, app: &ShellTuiApp, area: Rect, title: &str) {
    if let Some(ref todo_plan) = app.sidebar.todo_plan {
        let mut lines = Vec::new();
        // Use compact mode if height is small (e.g. when split view is active)
        let is_compact = area.height < 8;

        if is_compact {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} ({}/{})",
                    title,
                    todo_plan.completed_count(),
                    todo_plan.items.len()
                ),
                Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                title,
                Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
            )));

            let percent = todo_plan.progress_percent();
            let bar_width = area.width.saturating_sub(4) as usize;
            let filled = ((percent / 100.0) * bar_width as f32) as usize;
            let empty = bar_width.saturating_sub(filled);

            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled("█".repeat(filled), Style::default().fg(ACCENT_GREEN)),
                Span::styled("░".repeat(empty), Style::default().fg(TEXT_MUTED)),
            ]));

            lines.push(Line::from(Span::styled(
                format!(
                    " {}/{} tasks",
                    todo_plan.completed_count(),
                    todo_plan.items.len()
                ),
                Style::default().fg(TEXT_SECONDARY),
            )));
        }

        let header_height = if is_compact { 1 } else { 3 };
        let max_items = area.height.saturating_sub(header_height + 1) as usize; // +1 for scroll indicator
        let total_items = todo_plan.items.len();
        let in_progress_idx = todo_plan
            .items
            .iter()
            .position(|i| i.status == "in_progress");

        let scroll_start = if let Some(idx) = in_progress_idx {
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
            0
        };

        let visible_items: Vec<_> = todo_plan
            .items
            .iter()
            .skip(scroll_start)
            .take(max_items)
            .collect();

        if scroll_start > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↑ {} more above", scroll_start),
                Style::default().fg(TEXT_MUTED),
            )));
        }

        for item in visible_items.iter() {
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

            let max_len = area.width.saturating_sub(5) as usize;
            let desc = if item.content.len() > max_len {
                format!("{}...", &item.content[..max_len.saturating_sub(3)])
            } else {
                item.content.clone()
            };

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

        let items_below = total_items.saturating_sub(scroll_start + visible_items.len());
        if items_below > 0 {
            lines.push(Line::from(Span::styled(
                format!(" ↓ {} more below", items_below),
                Style::default().fg(TEXT_MUTED),
            )));
        }

        f.render_widget(Paragraph::new(lines), area);
    }
}

fn draw_tool_steps_section(f: &mut Frame, app: &ShellTuiApp, area: Rect, title: &str) {
    let tool_steps = &app.sidebar.tool_steps;
    let completed_count = app.sidebar.completed_tool_steps();
    let total_count = tool_steps.len();

    let mut lines = Vec::new();
    let is_compact = area.height < 8;

    if is_compact {
        lines.push(Line::from(Span::styled(
            format!("{} ({}/{})", title, completed_count, total_count),
            Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            title,
            Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
        )));

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

        lines.push(Line::from(Span::styled(
            format!(" {}/{} tools", completed_count, total_count),
            Style::default().fg(TEXT_SECONDARY),
        )));
    }

    let header_height = if is_compact { 1 } else { 3 };
    let max_items = area.height.saturating_sub(header_height + 1) as usize;
    let scroll_offset = app.sidebar.tool_steps_scroll_offset;

    let visible_steps: Vec<_> = tool_steps
        .iter()
        .rev()
        .skip(scroll_offset)
        .take(max_items)
        .collect();

    if scroll_offset > 0 {
        lines.push(Line::from(Span::styled(
            format!(" ↑ {} newer steps", scroll_offset.min(total_count)),
            Style::default().fg(TEXT_MUTED),
        )));
    }

    for step in visible_steps.iter() {
        let (icon, icon_color) = match step.status {
            ToolStepStatus::Completed => ("✓".to_string(), ACCENT_GREEN),
            ToolStepStatus::Running => {
                let spinner_chars = ["◐", "◓", "◑", "◒"];
                let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
                (spinner.to_string(), ACCENT_CYAN)
            }
            ToolStepStatus::Failed => ("✗".to_string(), ACCENT_RED),
        };

        let display_text = if step.description.is_empty() {
            step.tool_name.clone()
        } else {
            format!("{}: {}", step.tool_name, step.description)
        };

        let max_len = area.width.saturating_sub(5) as usize;
        let desc = if display_text.len() > max_len {
            format!("{}...", &display_text[..max_len.saturating_sub(3)])
        } else {
            display_text
        };

        let desc_style = match step.status {
            ToolStepStatus::Running => Style::default().fg(TEXT_PRIMARY),
            ToolStepStatus::Completed => Style::default().fg(TEXT_DIM),
            ToolStepStatus::Failed => Style::default().fg(ACCENT_RED),
        };

        lines.push(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("{} ", icon), Style::default().fg(icon_color)),
            Span::styled(desc, desc_style),
        ]));
    }

    let items_below = total_count.saturating_sub(scroll_offset + visible_steps.len());
    if items_below > 0 {
        lines.push(Line::from(Span::styled(
            format!(" ↓ {} older steps", items_below),
            Style::default().fg(TEXT_MUTED),
        )));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_thinking(f: &mut Frame, app: &ShellTuiApp, area: Rect, title: &str) {
    let spinner_chars = ["◐", "◓", "◑", "◒"];
    let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
    let thinking_word = app.spinner.current();
    let lines = vec![
        Line::from(Span::styled(
            title,
            Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(format!("{} ", spinner), Style::default().fg(ACCENT_CYAN)),
            Span::styled(thinking_word, Style::default().fg(TEXT_SECONDARY)),
        ]),
    ];

    f.render_widget(Paragraph::new(lines), area);
}

fn draw_empty_state(f: &mut Frame, title: &str, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            title,
            Style::default().fg(TEXT_DIM).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(" No content", Style::default().fg(TEXT_MUTED))),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_sidebar_lsp(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let connections = &app.sidebar.connections;
    let mut lines = Vec::new();

    // LSP header with count
    if connections.lsp_servers.is_empty() {
        if app.lsp_initializing {
            let spinner_chars = ["◐", "◓", "◑", "◒"];
            let spinner = spinner_chars[app.animation_frame % spinner_chars.len()];
            lines.push(Line::from(Span::styled(
                format!(" LSP {}", spinner),
                Style::default().fg(TEXT_DIM),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                " LSP ○",
                Style::default().fg(TEXT_MUTED),
            )));
        }
    } else {
        let connected_count = connections.lsp_servers.iter().filter(|(_, c)| *c).count();
        let total = connections.lsp_servers.len();
        let color = if connected_count == total { ACCENT_GREEN } else { ACCENT_YELLOW };

        lines.push(Line::from(Span::styled(
            format!(" LSP {}/{}", connected_count, total),
            Style::default().fg(color),
        )));

        // Show servers with status icons
        for (name, connected) in connections.lsp_servers.iter().take(2) {
            let (icon, color) = if *connected {
                ("●", ACCENT_GREEN)
            } else {
                ("○", ACCENT_RED)
            };
            let max_len = area.width.saturating_sub(4) as usize;
            let display = if name.len() > max_len {
                format!("{}…", &name[..max_len.saturating_sub(1)])
            } else {
                name.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(" ", Style::default()),
                Span::styled(icon, Style::default().fg(color)),
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

fn draw_model_picker_popup(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let filtered = app.model_picker.filtered_models();
    if filtered.is_empty() && app.model_picker.filter.is_empty() {
        return;
    }

    let max_entries = 12;
    let height = (filtered.len().min(max_entries) + 4) as u16;
    let width = 55.min(area.width.saturating_sub(10));

    let popup_area = Rect {
        x: (area.width.saturating_sub(width)) / 2,
        y: (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let provider_name = match app.model_picker.provider {
        crate::config::LlmProvider::Anthropic => "Anthropic",
        crate::config::LlmProvider::OpenAI => "OpenAI",
        crate::config::LlmProvider::GitHubCopilot => "GitHub Copilot",
        crate::config::LlmProvider::OpenRouter => "OpenRouter",
        crate::config::LlmProvider::Ollama => "Ollama",
    };

    let block = Block::default()
        .title(format!(" Select Model ({}) ", provider_name))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_MAGENTA))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Filter input
    let filter_area = Rect { height: 1, ..inner };
    let filter_text = if app.model_picker.filter.is_empty() {
        "Type to filter...".to_string()
    } else {
        app.model_picker.filter.clone()
    };

    let filter_para = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(ACCENT_CYAN)),
        Span::styled(
            filter_text,
            if app.model_picker.filter.is_empty() {
                Style::default().fg(TEXT_MUTED)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            },
        ),
    ]));
    f.render_widget(filter_para, filter_area);

    // Model list
    let list_area = Rect {
        y: inner.y + 1,
        height: inner.height.saturating_sub(2),
        ..inner
    };

    if filtered.is_empty() {
        let no_match = Paragraph::new("No matching models").style(Style::default().fg(TEXT_MUTED));
        f.render_widget(no_match, list_area);
        return;
    }

    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .take(max_entries)
        .map(|(i, model)| {
            let is_active = model.is_active;
            let style = if i == app.model_picker.selected {
                Style::default()
                    .fg(BG_PRIMARY)
                    .bg(ACCENT_MAGENTA)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(ACCENT_GREEN)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };

            let active_marker = if is_active { "✓ " } else { "  " };
            let desc = model.description.as_deref().unwrap_or("");
            let text = if desc.is_empty() {
                format!("{}{}", active_marker, model.name)
            } else {
                format!("{}{} - {}", active_marker, model.name, desc)
            };
            ListItem::new(text).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, list_area);

    // Help text at bottom
    let help_area = Rect {
        y: inner.y + inner.height - 1,
        height: 1,
        ..inner
    };
    let help = Paragraph::new(Line::from(vec![
        Span::styled("↑↓", Style::default().fg(ACCENT_CYAN)),
        Span::styled(" navigate  ", Style::default().fg(TEXT_MUTED)),
        Span::styled("Enter", Style::default().fg(ACCENT_CYAN)),
        Span::styled(" select  ", Style::default().fg(TEXT_MUTED)),
        Span::styled("Esc", Style::default().fg(ACCENT_CYAN)),
        Span::styled(" cancel", Style::default().fg(TEXT_MUTED)),
    ]));
    f.render_widget(help, help_area);
}

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

/// Draw command autocomplete popup for slash commands
fn draw_command_autocomplete_popup(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let suggestions = &app.command_autocomplete.suggestions;
    if suggestions.is_empty() {
        return;
    }

    // Calculate popup position and size
    let popup_height = (suggestions.len() as u16 + 2).min(10); // +2 for borders, max 10 items
    let popup_width = (suggestions
        .iter()
        .map(|s| s.command.len() + s.description.len() + 4) // Extra space for formatting
        .max()
        .unwrap_or(40) as u16)
        .min(80)
        .max(40);

    // Position above the input line
    let popup_area = Rect {
        x: 2,                                            // Align with input area
        y: area.height.saturating_sub(popup_height + 4), // Above input area (4 = input height estimate)
        width: popup_width,
        height: popup_height,
    };

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Create block with title
    let block = Block::default()
        .title(" Commands ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_ACCENT))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Create list items
    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .map(|(i, cmd)| {
            let is_selected = i == app.command_autocomplete.selected;

            let style = if is_selected {
                Style::default()
                    .bg(ACCENT_BLUE)
                    .fg(BG_PRIMARY)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };

            ListItem::new(Line::from(vec![
                Span::styled(
                    &cmd.command,
                    if is_selected {
                        Style::default()
                            .bg(ACCENT_BLUE)
                            .fg(BG_PRIMARY)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(ACCENT_GREEN)
                            .add_modifier(Modifier::BOLD)
                    },
                ),
                Span::styled(" - ", style),
                Span::styled(
                    &cmd.description,
                    if is_selected {
                        Style::default().bg(ACCENT_BLUE).fg(BG_PRIMARY)
                    } else {
                        Style::default().fg(TEXT_SECONDARY)
                    },
                ),
            ]))
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

/// Draw plan approval popup for Plan mode
fn draw_plan_approval_popup(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    use crate::planning::PlanStepStatus;

    // Calculate modal size - centered, sized to fit content (larger modal)
    let modal_width = (area.width as f32 * 0.85).min(100.0) as u16;

    // Calculate height based on content (header + steps + footer)
    let step_count = app.pending_approval_plan.as_ref().map(|p| p.steps.len()).unwrap_or(0);
    // Header (2) + plan title (2) + "Steps:" (1) + steps + total (2) + footer (3) + padding (2)
    let content_height = 2 + 2 + 1 + step_count + 2 + 3 + 2;
    let modal_height = ((content_height as u16) + 2) // +2 for borders
        .min((area.height as f32 * 0.9) as u16) // Use 90% of screen height
        .max(18); // Minimum height

    let popup_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Create the modal block
    let block = Block::default()
        .title(" Plan Approval ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Calculate how many steps we can show (reserve space for header + footer)
    let available_lines = inner.height as usize;
    let header_lines = 5; // Header + plan title + empty + "Steps:" + empty
    let footer_lines = 4; // Total + empty + key hints
    let max_visible_steps = available_lines.saturating_sub(header_lines + footer_lines);

    // Build content
    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(vec![Span::styled(
        "Review the plan before execution",
        Style::default()
            .fg(TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(""));

    // Plan info
    if let Some(ref plan) = app.pending_approval_plan {
        // Title
        lines.push(Line::from(vec![
            Span::styled("Plan: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(
                &plan.title,
                Style::default()
                    .fg(ACCENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(""));

        // Steps
        lines.push(Line::from(Span::styled(
            "Steps:",
            Style::default()
                .fg(TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )));

        let steps_to_show = plan.steps.len().min(max_visible_steps);
        for (i, step) in plan.steps.iter().take(steps_to_show).enumerate() {
            let status_icon = match step.status {
                PlanStepStatus::Pending => "○",
                PlanStepStatus::InProgress => "◐",
                PlanStepStatus::Completed => "✓",
                PlanStepStatus::Failed => "✗",
                PlanStepStatus::Skipped => "−",
            };

            // Truncate description if too long
            let max_desc_len = (modal_width as usize).saturating_sub(10);
            let description = if step.description.len() > max_desc_len {
                format!("{}...", &step.description[..max_desc_len.saturating_sub(3)])
            } else {
                step.description.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {} {}. ", status_icon, i + 1),
                    Style::default().fg(TEXT_SECONDARY),
                ),
                Span::styled(description, Style::default().fg(TEXT_PRIMARY)),
            ]));
        }

        // Show "and X more..." if there are hidden steps
        if plan.steps.len() > steps_to_show {
            let remaining = plan.steps.len() - steps_to_show;
            lines.push(Line::from(Span::styled(
                format!("     ... and {} more steps", remaining),
                Style::default().fg(TEXT_DIM),
            )));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("Total: {} steps", plan.steps.len()),
            Style::default().fg(TEXT_SECONDARY),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "Loading plan details...",
            Style::default().fg(TEXT_SECONDARY),
        )));
    }

    // Footer with key hints - always visible
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            " Y ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Approve  ", Style::default().fg(TEXT_PRIMARY)),
        Span::styled(
            " N ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT_RED)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Reject  ", Style::default().fg(TEXT_PRIMARY)),
        Span::styled(
            " Esc ",
            Style::default()
                .fg(Color::Black)
                .bg(TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Cancel", Style::default().fg(TEXT_PRIMARY)),
    ]));

    // Render paragraph
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

/// Draw tool approval modal (Codex CLI style)
fn draw_tool_approval_modal(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let approval = match &app.pending_tool_approval {
        Some(a) => a,
        None => return,
    };

    // Calculate modal size
    let modal_width = (area.width as f32 * 0.7).min(80.0) as u16;
    let modal_height = 14u16;

    let popup_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Create the modal block with appropriate border color
    let border_color = if approval.high_risk { ACCENT_RED } else { ACCENT_YELLOW };
    let title = if approval.high_risk {
        " ⚠ High-Risk Action "
    } else {
        " Tool Approval Required "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG_BLOCK));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Build content
    let mut lines: Vec<Line> = Vec::new();

    // Header
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Tool: ", Style::default().fg(TEXT_SECONDARY)),
        Span::styled(
            &approval.tool_name,
            Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD),
        ),
    ]));

    // Description
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Action: ", Style::default().fg(TEXT_SECONDARY)),
        Span::styled(&approval.description, Style::default().fg(TEXT_PRIMARY)),
    ]));

    // Args preview (truncated if long)
    if !approval.args_preview.is_empty() {
        lines.push(Line::from(""));
        let preview = if approval.args_preview.len() > (inner.width as usize - 10) {
            format!("{}...", &approval.args_preview[..inner.width as usize - 13])
        } else {
            approval.args_preview.clone()
        };
        lines.push(Line::from(vec![
            Span::styled("Preview: ", Style::default().fg(TEXT_SECONDARY)),
            Span::styled(preview, Style::default().fg(TEXT_DIM)),
        ]));
    }

    // Spacer
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Key hints
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            " Y ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Allow  ", Style::default().fg(TEXT_PRIMARY)),
        Span::styled(
            " A ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT_CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Allow All  ", Style::default().fg(TEXT_PRIMARY)),
        Span::styled(
            " N ",
            Style::default()
                .fg(Color::Black)
                .bg(ACCENT_RED)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" Deny", Style::default().fg(TEXT_PRIMARY)),
    ]));

    // Render
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}
