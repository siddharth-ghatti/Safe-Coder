//! Shell-first TUI rendering
//!
//! Renders the Warp-like shell interface with command blocks,
//! status bar, and shell prompt input.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};
use similar::{ChangeTag, TextDiff};
use textwrap::wrap;

use super::shell_app::{BlockOutput, BlockType, CommandBlock, FileDiff, InputMode, ShellTuiApp};

// Crush-inspired color scheme
const ACCENT_MAGENTA: Color = Color::Rgb(200, 100, 200); // Magenta for AI responses
const ACCENT_GREEN: Color = Color::Rgb(100, 200, 140); // Green for success/user input
const ACCENT_AMBER: Color = Color::Rgb(220, 180, 100); // Amber for tools/warnings
const ACCENT_RED: Color = Color::Rgb(220, 100, 100); // Red for errors/deletions
const ACCENT_CYAN: Color = Color::Rgb(100, 200, 200); // Cyan for info/paths

const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 220); // Main text
const TEXT_DIM: Color = Color::Rgb(100, 100, 110); // Dimmed text
const TEXT_MUTED: Color = Color::Rgb(70, 70, 80); // Very dim text

const BG_DARK: Color = Color::Rgb(20, 20, 25); // Dark background
const BORDER_DIM: Color = Color::Rgb(50, 50, 55); // Subtle borders

/// Draw the complete shell TUI
pub fn draw(f: &mut Frame, app: &mut ShellTuiApp) {
    let size = f.area();

    // Layout: status bar (top), blocks (middle), input (bottom)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Status bar
            Constraint::Min(3),    // Command blocks
            Constraint::Length(3), // Input area
        ])
        .split(size);

    draw_status_bar(f, app, main_layout[0]);
    draw_blocks(f, app, main_layout[1]);
    draw_input(f, app, main_layout[2]);

    // Draw autocomplete popup on top if visible
    if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
        draw_autocomplete(f, app, main_layout[2]);
    }
}

/// Draw the status bar at the top
fn draw_status_bar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Directory
    let cwd_display = app
        .cwd
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| app.cwd.display().to_string());

    // Git branch
    let git_branch = app.get_git_branch();

    let mut status_parts = vec![format!(" {} ", cwd_display)];

    if let Some(ref branch) = git_branch {
        status_parts.push(format!("({}) ", branch));
    }

    if app.ai_connected {
        status_parts.push("◇ AI ".to_string());
    }

    let running_count = app.blocks.iter().filter(|b| b.is_running()).count();
    if running_count > 0 {
        let dots = ".".repeat((app.animation_frame / 10) % 4);
        status_parts.push(format!("working{} ", dots));
    }

    let left_text = status_parts.join("");
    let right_text = "safe-coder";
    let total_len = left_text.len() + right_text.len();
    let padding = if (area.width as usize) > total_len {
        " ".repeat(area.width as usize - total_len)
    } else {
        String::new()
    };

    let full_status = format!("{}{}{}", left_text, padding, right_text);

    let status = Paragraph::new(full_status)
        .style(Style::default().fg(TEXT_DIM).bg(BG_DARK));

    f.render_widget(status, area);
}

/// Draw command blocks
fn draw_blocks(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    let content_width = (area.width.saturating_sub(4)) as usize;
    let content_width = content_width.max(20);

    // Build all lines from blocks
    let mut all_lines: Vec<String> = Vec::new();

    for block in app.blocks.iter() {
        render_block_to_strings(block, content_width, &mut all_lines, app.animation_frame);
        all_lines.push(String::new()); // Gap between blocks
    }

    // Calculate visible portion (scroll_offset = 0 shows bottom)
    let max_lines = area.height as usize;
    let total_lines = all_lines.len();

    let visible_start = if total_lines > max_lines {
        total_lines
            .saturating_sub(max_lines)
            .saturating_sub(app.scroll_offset)
    } else {
        0
    };
    let visible_end = (visible_start + max_lines).min(total_lines);

    let visible_items: Vec<ListItem> = all_lines
        .get(visible_start..visible_end)
        .unwrap_or(&[])
        .iter()
        .map(|s| {
            // Apply colors based on line prefix markers
            let line = colorize_line(s);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(visible_items);
    f.render_widget(list, area);

    // Scrollbar
    if total_lines > max_lines {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        let scroll_pos = total_lines
            .saturating_sub(max_lines)
            .saturating_sub(app.scroll_offset);

        let mut scrollbar_state =
            ScrollbarState::new(total_lines.saturating_sub(max_lines)).position(scroll_pos);

        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };

        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

/// Colorize a line based on embedded markers
fn colorize_line(s: &str) -> Line<'static> {
    // Parse marker prefix and apply appropriate styling
    if s.starts_with("│M ") {
        // Magenta left border for AI content
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_MAGENTA)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│G ") {
        // Green left border for user/success
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_GREEN)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│A ") {
        // Amber left border for tools
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_DIM)),
        ])
    } else if s.starts_with("│R ") {
        // Red left border for errors
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_RED)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│D-") {
        // Diff deletion (red)
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!(" - {}", content), Style::default().fg(ACCENT_RED)),
        ])
    } else if s.starts_with("│D+") {
        // Diff addition (green)
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!(" + {}", content), Style::default().fg(ACCENT_GREEN)),
        ])
    } else if s.starts_with("│D ") {
        // Diff context
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!("   {}", content), Style::default().fg(TEXT_MUTED)),
        ])
    } else if s.starts_with("│T ") {
        // Tool header
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(ACCENT_AMBER)),
        ])
    } else if s.starts_with("│_ ") {
        // Dim/muted content
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(TEXT_MUTED)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_MUTED)),
        ])
    } else if s.starts_with("> ") {
        // Shell command prompt
        let content = &s[2..];
        Line::from(vec![
            Span::styled("> ", Style::default().fg(ACCENT_GREEN).add_modifier(Modifier::BOLD)),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("@ ") {
        // AI query prompt
        let content = &s[2..];
        Line::from(vec![
            Span::styled("@ ", Style::default().fg(ACCENT_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else {
        Line::from(s.to_string())
    }
}

/// Render a single command block to plain strings with marker prefixes
fn render_block_to_strings(
    block: &CommandBlock,
    width: usize,
    lines: &mut Vec<String>,
    animation_frame: usize,
) {
    match &block.block_type {
        BlockType::SystemMessage => {
            let output = block.output.get_text();
            for line in output.lines() {
                let wrapped = wrap(line, width.saturating_sub(4));
                for wrapped_line in wrapped {
                    lines.push(format!("│_ {}", wrapped_line));
                }
            }
        }

        BlockType::ShellCommand => {
            // Shell command header
            let mut header = format!("> {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  {}", dots));
            } else if let Some(code) = block.exit_code {
                if code != 0 {
                    header.push_str(&format!(" ✗ {}", code));
                }
            }

            lines.push(header);

            // Output with green border
            let output = block.output.get_text();
            if !output.is_empty() {
                for line in output.lines().take(50) {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for wrapped_line in wrapped {
                        lines.push(format!("│G {}", wrapped_line));
                    }
                }
                if output.lines().count() > 50 {
                    lines.push("│G ... [truncated]".to_string());
                }
            }
        }

        BlockType::AiQuery => {
            // AI query header
            let mut header = format!("@ {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  thinking{}", dots));
            }

            lines.push(header);
            lines.push(String::new());

            // Render tool executions first (child blocks)
            if !block.children.is_empty() {
                for child in &block.children {
                    render_tool_strings(child, width, lines, animation_frame);
                    lines.push(String::new());
                }
            }

            // Render final AI response (magenta border) - only if there's actual content
            let output = block.output.get_text();
            if !output.is_empty() && !block.is_running() {
                // Add separator if there were tools
                if !block.children.is_empty() {
                    lines.push("│M ─────────────────────────────────".to_string());
                }

                for line in output.lines() {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for wrapped_line in wrapped {
                        lines.push(format!("│M {}", wrapped_line));
                    }
                }
            } else if block.is_running() && output.is_empty() && block.children.is_empty() {
                // Show thinking indicator only if no tools yet
                lines.push("│M ...".to_string());
            }
        }

        BlockType::AiToolExecution { .. } => {
            render_tool_strings(block, width, lines, animation_frame);
        }

        BlockType::Orchestration => {
            let mut header = format!("⚙ orchestrate {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  {}", dots));
            }

            lines.push(header);

            let output = block.output.get_text();
            if !output.is_empty() {
                for line in output.lines() {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for wrapped_line in wrapped {
                        lines.push(format!("│A {}", wrapped_line));
                    }
                }
            }
        }
    }
}

/// Render tool execution block strings
fn render_tool_strings(
    block: &CommandBlock,
    width: usize,
    lines: &mut Vec<String>,
    animation_frame: usize,
) {
    let tool_name = match &block.block_type {
        BlockType::AiToolExecution { tool_name } => tool_name.clone(),
        _ => "tool".to_string(),
    };

    // Tool header with amber color
    let mut header = format!("⚡ {}", tool_name);

    // Add file path or description from input
    if !block.input.is_empty() {
        header.push_str(&format!(" {}", block.input));
    }

    // Status indicator
    if block.is_running() {
        let dots = ".".repeat((animation_frame / 10) % 4);
        header.push_str(&format!(" {}", dots));
    } else if let Some(exit_code) = block.exit_code {
        if exit_code == 0 {
            header.push_str(" ✓");
        } else {
            header.push_str(" ✗");
        }
    }

    lines.push(format!("│T {}", header));

    // Render diff if present (for edit_file/write_file tools)
    if let Some(diff) = &block.diff {
        render_diff_strings(diff, width, lines);
    } else {
        // Render regular output (truncated)
        let output = block.output.get_text();
        if !output.is_empty() {
            // Show compact output
            let output_lines: Vec<&str> = output.lines().take(5).collect();
            for line in output_lines {
                let truncated = if line.len() > width.saturating_sub(6) {
                    format!("{}...", &line[..width.saturating_sub(9)])
                } else {
                    line.to_string()
                };
                lines.push(format!("│_ {}", truncated));
            }
            if output.lines().count() > 5 {
                lines.push(format!("│_ ... ({} more lines)", output.lines().count() - 5));
            }
        }
    }
}

/// Render a file diff with color-coded additions and deletions
fn render_diff_strings(diff: &FileDiff, width: usize, lines: &mut Vec<String>) {
    // File path header
    lines.push(format!("│A 📝 {}", diff.path));

    // Compute the diff
    let text_diff = TextDiff::from_lines(&diff.old_content, &diff.new_content);

    let inner_width = width.saturating_sub(8);
    let mut has_changes = false;
    let mut change_count = 0;

    for change in text_diff.iter_all_changes() {
        if change_count >= 20 {
            break;
        }

        let line_content = change.value().trim_end();

        // Truncate long lines
        let display_content = if line_content.len() > inner_width {
            format!("{}...", &line_content[..inner_width.saturating_sub(3)])
        } else {
            line_content.to_string()
        };

        match change.tag() {
            ChangeTag::Delete => {
                has_changes = true;
                lines.push(format!("│D-{}", display_content));
                change_count += 1;
            }
            ChangeTag::Insert => {
                has_changes = true;
                lines.push(format!("│D+{}", display_content));
                change_count += 1;
            }
            ChangeTag::Equal => {
                // Skip context lines for cleaner output
            }
        }
    }

    if !has_changes {
        lines.push("│_ (no changes)".to_string());
    } else if text_diff.iter_all_changes().filter(|c| c.tag() != ChangeTag::Equal).count() > 20 {
        lines.push("│_ ... (more changes)".to_string());
    }
}

/// Draw the input area at the bottom
fn draw_input(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Top border
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER_DIM));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Build prompt string
    let cwd_display = app
        .cwd
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "~".to_string());

    let git_branch = app.get_git_branch();

    let branch_part = if let Some(ref branch) = git_branch {
        format!(" ({})", branch)
    } else {
        String::new()
    };

    // Build spans for prompt
    let mut spans = Vec::new();

    spans.push(Span::styled(cwd_display, Style::default().fg(ACCENT_CYAN)));

    if !branch_part.is_empty() {
        spans.push(Span::styled(branch_part, Style::default().fg(TEXT_DIM)));
    }

    let prompt_color = if app.last_exit_code == 0 {
        ACCENT_GREEN
    } else {
        ACCENT_RED
    };

    let prompt_char = if matches!(app.input_mode, InputMode::AiPrefix) {
        "@"
    } else {
        ">"
    };

    spans.push(Span::styled(
        format!(" {} ", prompt_char),
        Style::default()
            .fg(prompt_color)
            .add_modifier(Modifier::BOLD),
    ));

    // Input with cursor
    let (before_cursor, after_cursor) = app.input.split_at(app.cursor_pos.min(app.input.len()));

    let input_color = match app.input_mode {
        InputMode::AiPrefix => ACCENT_MAGENTA,
        _ => TEXT_PRIMARY,
    };

    spans.push(Span::styled(
        before_cursor.to_string(),
        Style::default().fg(input_color),
    ));

    // Cursor
    let cursor_char = if app.animation_frame % 20 < 10 {
        if after_cursor.is_empty() {
            "█"
        } else {
            &after_cursor[..1]
        }
    } else {
        if after_cursor.is_empty() {
            " "
        } else {
            &after_cursor[..1]
        }
    };

    spans.push(Span::styled(
        cursor_char.to_string(),
        Style::default()
            .fg(input_color)
            .add_modifier(Modifier::REVERSED),
    ));

    if after_cursor.len() > 1 {
        spans.push(Span::styled(
            after_cursor[1..].to_string(),
            Style::default().fg(input_color),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans));

    let input_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    f.render_widget(paragraph, input_area);
}

/// Draw the autocomplete popup above the input area
fn draw_autocomplete(f: &mut Frame, app: &ShellTuiApp, input_area: Rect) {
    let suggestions = &app.autocomplete.suggestions;
    let selected = app.autocomplete.selected;

    if suggestions.is_empty() {
        return;
    }

    // Calculate popup dimensions
    let max_width = suggestions.iter().map(|s| s.len()).max().unwrap_or(10) + 4;
    let width = (max_width as u16)
        .min(input_area.width.saturating_sub(4))
        .max(15);
    let height = (suggestions.len() as u16 + 2).min(12);

    // Position popup above the input area
    let x = input_area.x + 2;
    let y = input_area.y.saturating_sub(height);

    let popup_area = Rect {
        x,
        y,
        width,
        height,
    };

    // Create popup block with border
    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_DARK));

    let inner = popup_block.inner(popup_area);

    // Clear the area first
    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(popup_block, popup_area);

    // Render suggestions
    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .take(10)
        .map(|(i, suggestion)| {
            let style = if i == selected {
                Style::default()
                    .fg(BG_DARK)
                    .bg(ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };

            // Add icon based on type
            let icon = if suggestion.ends_with('/') {
                "📁 "
            } else if suggestion.contains('.') {
                "📄 "
            } else {
                "⚡ "
            };

            ListItem::new(format!("{}{}", icon, suggestion)).style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}
