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
use textwrap::wrap;

use super::shell_app::{BlockType, CommandBlock, InputMode, ShellTuiApp};

// Color scheme - consistent with existing TUI
const ACCENT_BLUE: Color = Color::Rgb(100, 149, 237); // Cornflower blue for shell
const ACCENT_PURPLE: Color = Color::Rgb(180, 120, 200); // Purple for AI
const ACCENT_GREEN: Color = Color::Rgb(120, 200, 140); // Green for success
const ACCENT_AMBER: Color = Color::Rgb(220, 180, 100); // Amber for tools/warnings
const ACCENT_RED: Color = Color::Rgb(220, 100, 100); // Red for errors
const ACCENT_CYAN: Color = Color::Rgb(100, 200, 200); // Cyan for info

const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 220); // Main text
const TEXT_DIM: Color = Color::Rgb(120, 120, 120); // Dimmed text

const BORDER_DIM: Color = Color::Rgb(60, 60, 65); // Subtle borders

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
}

/// Draw the status bar at the top
fn draw_status_bar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Directory
    let cwd_display = app
        .cwd
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| app.cwd.display().to_string());

    // Git branch - get it first to avoid lifetime issues
    let git_branch = app.get_git_branch();

    let mut status_parts = vec![format!(" {} ", cwd_display)];

    if let Some(ref branch) = git_branch {
        status_parts.push(format!("({}) ", branch));
    }

    if app.ai_connected {
        status_parts.push("[AI] ".to_string());
    }

    let running_count = app.blocks.iter().filter(|b| b.is_running()).count();
    if running_count > 0 {
        let dots = ".".repeat((app.animation_frame / 10) % 4);
        status_parts.push(format!("running{} ", dots));
    }

    let left_text = status_parts.join("");
    let right_text = "safe-coder";
    let padding = area.width as usize - left_text.len() - right_text.len();
    let padding_str = if padding > 0 {
        " ".repeat(padding)
    } else {
        String::new()
    };

    let full_status = format!("{}{}{}", left_text, padding_str, right_text);

    let status = Paragraph::new(full_status)
        .style(Style::default().fg(TEXT_PRIMARY).bg(Color::Rgb(25, 25, 30)));

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
        .map(|s| ListItem::new(Line::from(s.clone())))
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

/// Render a single command block to plain strings (avoids lifetime issues)
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
                let wrapped = wrap(line, width);
                for wrapped_line in wrapped {
                    lines.push(format!("  {}", wrapped_line));
                }
            }
        }

        BlockType::ShellCommand => {
            let mut header = format!("> {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  running{}", dots));
            } else if let Some(duration) = block.duration_display() {
                header.push_str(&format!("  [{}]", duration));
            }

            if let Some(code) = block.exit_code {
                if code != 0 {
                    header.push_str(&format!(" ✗{}", code));
                }
            }

            lines.push(header);
            render_output_strings(block, width, lines);
        }

        BlockType::AiQuery => {
            let mut header = format!("@ {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  thinking{}", dots));
            } else {
                header.push_str("  [AI]");
            }

            lines.push(header);
            render_output_strings(block, width, lines);

            // Render child blocks (tool executions)
            for child in &block.children {
                lines.push(String::new());
                render_tool_strings(child, width, lines, animation_frame);
            }
        }

        BlockType::AiToolExecution { .. } => {
            render_tool_strings(block, width, lines, animation_frame);
        }

        BlockType::Orchestration => {
            let mut header = format!("⚙ orchestrate {}", block.input);

            if block.is_running() {
                let dots = ".".repeat((animation_frame / 10) % 4);
                header.push_str(&format!("  running{}", dots));
            }

            lines.push(header);
            render_output_strings(block, width, lines);
        }
    }
}

/// Render output as bordered block strings
fn render_output_strings(block: &CommandBlock, width: usize, lines: &mut Vec<String>) {
    let output = block.output.get_text();
    if output.is_empty() && !block.is_running() {
        return;
    }

    let inner_width = width.saturating_sub(4);

    // Top border
    lines.push(format!("  ┌{}┐", "─".repeat(inner_width)));

    if output.is_empty() && block.is_running() {
        let padding = inner_width.saturating_sub(4);
        lines.push(format!("  │ ...{}│", " ".repeat(padding)));
    } else {
        for line in output.lines().take(50) {
            let wrapped = wrap(line, inner_width.saturating_sub(2));
            for wrapped_line in wrapped {
                let content = wrapped_line.to_string();
                let padding = inner_width.saturating_sub(content.len() + 1);
                lines.push(format!("  │ {}{}│", content, " ".repeat(padding)));
            }
        }

        if output.lines().count() > 50 {
            let msg = "... [output truncated]";
            let padding = inner_width.saturating_sub(msg.len() + 1);
            lines.push(format!("  │ {}{}│", msg, " ".repeat(padding)));
        }
    }

    // Bottom border
    lines.push(format!("  └{}┘", "─".repeat(inner_width)));
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

    let mut header = format!("    ⚡ {}", tool_name);

    if !block.input.is_empty() {
        header.push_str(&format!(" {}", block.input));
    }

    if block.is_running() {
        let dots = ".".repeat((animation_frame / 10) % 4);
        header.push_str(&format!(" running{}", dots));
    }

    lines.push(header);

    let output = block.output.get_text();
    if !output.is_empty() {
        let inner_width = width.saturating_sub(8);
        for line in output.lines().take(10) {
            let wrapped = wrap(line, inner_width);
            for wrapped_line in wrapped {
                lines.push(format!("      {}", wrapped_line));
            }
        }
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

    // Get git branch to avoid lifetime issues
    let git_branch = app.get_git_branch();

    let branch_part = if let Some(ref branch) = git_branch {
        format!(" ({})", branch)
    } else {
        String::new()
    };

    let prompt_symbol = if app.last_exit_code == 0 { ">" } else { ">" };

    // Build spans for prompt
    let mut spans = Vec::new();

    spans.push(Span::styled(cwd_display, Style::default().fg(ACCENT_CYAN)));

    if !branch_part.is_empty() {
        spans.push(Span::styled(branch_part, Style::default().fg(ACCENT_AMBER)));
    }

    let prompt_color = if app.last_exit_code == 0 {
        ACCENT_GREEN
    } else {
        ACCENT_RED
    };
    spans.push(Span::styled(
        format!(" {} ", prompt_symbol),
        Style::default()
            .fg(prompt_color)
            .add_modifier(Modifier::BOLD),
    ));

    // Input with cursor
    let (before_cursor, after_cursor) = app.input.split_at(app.cursor_pos.min(app.input.len()));

    let input_color = match app.input_mode {
        InputMode::AiPrefix => ACCENT_PURPLE,
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
