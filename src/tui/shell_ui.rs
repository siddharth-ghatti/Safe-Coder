//! Shell-first TUI rendering with Crush-inspired layout
//!
//! Features:
//! - Right sidebar with logo and session info
//! - Main content area on left with command blocks
//! - Status bar and input at bottom

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

use super::file_picker::FilePicker;
use super::shell_app::{BlockType, CommandBlock, FileDiff, PermissionMode, ShellTuiApp};

// Crush-inspired color scheme
const ACCENT_MAGENTA: Color = Color::Rgb(200, 100, 200); // Magenta for AI/logo
const ACCENT_GREEN: Color = Color::Rgb(100, 200, 140); // Green for success
const ACCENT_AMBER: Color = Color::Rgb(220, 180, 100); // Amber for tools
const ACCENT_RED: Color = Color::Rgb(220, 100, 100); // Red for errors
const ACCENT_CYAN: Color = Color::Rgb(100, 200, 200); // Cyan for info

const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 220); // Main text
const TEXT_DIM: Color = Color::Rgb(100, 100, 110); // Dimmed text
const TEXT_MUTED: Color = Color::Rgb(70, 70, 80); // Very dim text

const BG_DARK: Color = Color::Rgb(20, 20, 25); // Dark background
const BORDER_DIM: Color = Color::Rgb(50, 50, 55); // Subtle borders

// Animated spinner frames
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const PROGRESS_CHARS: &[&str] = &["░", "▒", "▓", "█", "▓", "▒"];
const PULSE_CHARS: &[&str] = &["◐", "◓", "◑", "◒"];
const THINKING_FRAMES: &[&str] = &["🧠", "💭", "💡", "✨"];

// Sidebar constraints
const SIDEBAR_MIN_WIDTH: u16 = 28;
const SIDEBAR_PREFERRED_WIDTH: u16 = 48; // Wide enough for full logo
const MIN_MAIN_WIDTH: u16 = 50; // Minimum main content area

/// Full ASCII art logo (needs ~45 chars width)
const LOGO_FULL: &str = r#"╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱
 ___        __         ___          _
/ __| __ _ / _|___    / __|___   __| |___ _ _
\__ \/ _` |  _/ -_)  | (__/ _ \ / _` / -_) '_|
|___/\__,_|_| \___|   \___\___/ \__,_\___|_|
╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱"#;

/// Compact logo for narrow terminals
const LOGO_COMPACT: &str = r#"╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱
  ___        __
 / __| __ _ / _|___
 \__ \/ _` |  _/ -_)
 |___/\__,_|_| \___|
    ___         _
   / __|___  __| |___ _ _
  | (__/ _ \/ _` / -_) '_|
   \___\___/\__,_\___|_|
╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱"#;

/// Minimal logo for very narrow terminals
const LOGO_MINIMAL: &str = r#"╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱
 Safe Coder
╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱"#;

/// Calculate sidebar width based on terminal size
fn calculate_sidebar_width(total_width: u16) -> u16 {
    // If terminal is too narrow, hide sidebar entirely
    if total_width < MIN_MAIN_WIDTH + SIDEBAR_MIN_WIDTH {
        return 0;
    }

    // Calculate available space for sidebar
    let available = total_width.saturating_sub(MIN_MAIN_WIDTH);

    // Use preferred width if we have space, otherwise use what's available
    available
        .min(SIDEBAR_PREFERRED_WIDTH)
        .max(SIDEBAR_MIN_WIDTH)
}

/// Get the appropriate logo based on sidebar width
fn get_logo_for_width(width: u16) -> &'static str {
    if width >= 46 {
        LOGO_FULL
    } else if width >= 26 {
        LOGO_COMPACT
    } else {
        LOGO_MINIMAL
    }
}

/// Draw the complete shell TUI with sidebar
pub fn draw(f: &mut Frame, app: &mut ShellTuiApp) {
    let size = f.area();

    // Calculate dynamic sidebar width
    let sidebar_width = calculate_sidebar_width(size.width);

    // If sidebar width is 0, just draw main content (narrow terminal)
    if sidebar_width == 0 {
        draw_main_content(f, app, size);

        if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
            let main_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(size);
            draw_autocomplete(f, app, main_layout[1]);
        }
        return;
    }

    // Main horizontal layout: content (left) | sidebar (right)
    let horizontal_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(MIN_MAIN_WIDTH),   // Main content area
            Constraint::Length(sidebar_width), // Right sidebar (dynamic)
        ])
        .split(size);

    // Draw main content area (left side)
    draw_main_content(f, app, horizontal_layout[0]);

    // Draw sidebar (right side)
    draw_sidebar(f, app, horizontal_layout[1]);

    // Draw autocomplete popup on top if visible
    if app.autocomplete.visible && !app.autocomplete.suggestions.is_empty() {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(horizontal_layout[0]);
        draw_autocomplete(f, app, main_layout[1]);
    }

    // Draw file picker dropup if visible
    if app.file_picker.visible {
        draw_file_picker(f, app, horizontal_layout[0]);
    }
}

/// Draw the main content area (left side)
fn draw_main_content(f: &mut Frame, app: &mut ShellTuiApp, area: Rect) {
    // Calculate input height based on content (for word wrap)
    // Be aggressive: use 70% of width to trigger wrap earlier and account for prompt/margins
    let effective_width = (area.width.saturating_sub(6) as usize * 70) / 100;
    let effective_width = effective_width.max(10); // minimum 10 chars
    let input_char_count = app.input.chars().count() + 2; // +2 for "> " prompt
    let input_lines = if effective_width > 0 {
        ((input_char_count / effective_width) + 1).max(1).min(6) as u16 // max 6 lines
    } else {
        1
    };
    let input_height = input_lines + 2; // +2 for border and helper text

    // Vertical layout: blocks (top), input (bottom)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),               // Command blocks
            Constraint::Length(input_height), // Input area (dynamic)
        ])
        .split(area);

    draw_blocks(f, app, main_layout[0]);
    draw_input(f, app, main_layout[1]);
}

/// Draw the right sidebar with logo and info
fn draw_sidebar(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Sidebar background
    let sidebar_block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(BORDER_DIM))
        .style(Style::default().bg(BG_DARK));

    f.render_widget(sidebar_block, area);

    // Calculate logo height based on which logo we'll use
    let logo = get_logo_for_width(area.width);
    let logo_height = logo.lines().count() as u16 + 1;

    // Sidebar content layout
    let sidebar_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(logo_height), // Logo (dynamic height)
            Constraint::Length(2),           // Session info
            Constraint::Length(2),           // Project path
            Constraint::Length(3),           // Model info
            Constraint::Length(4),           // Permission mode section
            Constraint::Length(5),           // Modified files section
            Constraint::Min(1),              // Spacer
            Constraint::Length(2),           // Help hints
        ])
        .margin(1)
        .split(Rect {
            x: area.x + 1, // Account for border
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        });

    // Draw logo (pass width to select appropriate logo)
    draw_logo(f, sidebar_layout[0], area.width, app.animation_frame);

    // Draw session info
    draw_session_info(f, app, sidebar_layout[1]);

    // Draw project path
    draw_project_path(f, app, sidebar_layout[2]);

    // Draw model info with animation
    draw_model_info(f, app, sidebar_layout[3], app.animation_frame);

    // Draw permission mode
    draw_permission_mode(f, app, sidebar_layout[4], app.animation_frame);

    // Draw modified files
    draw_modified_files(f, app, sidebar_layout[5]);

    // Draw help hints
    draw_help_hints(f, sidebar_layout[7]);
}

/// Draw the ASCII logo (selects appropriate size based on width)
fn draw_logo(f: &mut Frame, area: Rect, sidebar_width: u16, animation_frame: usize) {
    let logo = get_logo_for_width(sidebar_width);

    // Subtle color cycling for the logo - creates a gentle shimmer effect
    let cycle = animation_frame % 60;
    let base_r = 200u8;
    let base_g = 100u8;
    let base_b = 200u8;

    // Very subtle brightness variation
    let brightness_offset = if cycle < 30 { cycle } else { 60 - cycle } as i16;
    let r = (base_r as i16 + brightness_offset / 2).clamp(0, 255) as u8;
    let g = (base_g as i16 + brightness_offset / 3).clamp(0, 255) as u8;
    let b = (base_b as i16 + brightness_offset / 2).clamp(0, 255) as u8;

    let logo_color = Color::Rgb(r, g, b);

    let logo_lines: Vec<Line> = logo
        .lines()
        .map(|line| {
            Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(logo_color),
            ))
        })
        .collect();

    let logo = Paragraph::new(logo_lines);
    f.render_widget(logo, area);
}

/// Draw session info section
fn draw_session_info(f: &mut Frame, _app: &ShellTuiApp, area: Rect) {
    let lines = vec![Line::from(Span::styled(
        "New Session",
        Style::default().fg(TEXT_PRIMARY),
    ))];

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Draw project path section
fn draw_project_path(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let path_display = app
        .cwd
        .to_string_lossy()
        .to_string()
        .replace(std::env::var("HOME").unwrap_or_default().as_str(), "~");

    // Truncate if too long
    let max_len = area.width as usize - 2;
    let display = if path_display.len() > max_len {
        format!("...{}", &path_display[path_display.len() - max_len + 3..])
    } else {
        path_display
    };

    let lines = vec![Line::from(Span::styled(
        display,
        Style::default().fg(TEXT_DIM),
    ))];

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Draw model info section with animated status
fn draw_model_info(f: &mut Frame, app: &ShellTuiApp, area: Rect, animation_frame: usize) {
    let model_name = if app.ai_connected {
        "Claude Sonnet"
    } else {
        "Not connected"
    };

    // Animated status indicator
    let (status, status_indicator) = if app.ai_thinking {
        let thinking_icon = THINKING_FRAMES[(animation_frame / 3) % THINKING_FRAMES.len()];
        let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
        (
            format!("{} Thinking {}", thinking_icon, spinner),
            ACCENT_AMBER,
        )
    } else if app.ai_connected {
        // Gentle pulsing for ready state
        let pulse = PULSE_CHARS[(animation_frame / 5) % PULSE_CHARS.len()];
        (format!("{} Ready", pulse), ACCENT_GREEN)
    } else {
        ("○ Offline".to_string(), TEXT_MUTED)
    };

    // Animated model indicator when connected
    let model_indicator = if app.ai_connected {
        let cycle = animation_frame % 40;
        if cycle < 20 {
            "◆"
        } else {
            "◇"
        }
    } else {
        "◇"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", model_indicator),
                Style::default().fg(ACCENT_MAGENTA),
            ),
            Span::styled(model_name, Style::default().fg(TEXT_PRIMARY)),
        ]),
        Line::from(Span::styled(status, Style::default().fg(status_indicator))),
    ];

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Draw permission mode section with animated indicator
fn draw_permission_mode(f: &mut Frame, app: &ShellTuiApp, area: Rect, animation_frame: usize) {
    let mode = app.permission_mode;

    // Mode-specific colors and icons
    let (mode_color, mode_icon) = match mode {
        PermissionMode::Yolo => (ACCENT_RED, "👹"),
        PermissionMode::Edit => (ACCENT_AMBER, "✏"),
        PermissionMode::Ask => (ACCENT_GREEN, "🛡"),
    };

    // Subtle pulsing animation for the mode indicator
    let pulse = if animation_frame % 30 < 15 {
        "●"
    } else {
        "○"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Mode ", Style::default().fg(TEXT_DIM)),
            Span::styled("───────────", Style::default().fg(BORDER_DIM)),
        ]),
        Line::from(vec![
            Span::styled(format!("{} ", mode_icon), Style::default().fg(mode_color)),
            Span::styled(
                mode.short_name(),
                Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {}", pulse), Style::default().fg(mode_color)),
        ]),
        Line::from(Span::styled(
            format!("ctrl+p: {}", mode.description()),
            Style::default().fg(TEXT_MUTED),
        )),
    ];

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Draw modified files section
fn draw_modified_files(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    // Count unique files that were edited in recent tool calls
    let mut seen = std::collections::HashSet::new();
    let modified_files: Vec<String> = app
        .blocks
        .iter()
        .flat_map(|b| &b.children)
        .filter_map(|child| child.diff.as_ref().map(|d| d.path.clone()))
        .filter(|path| seen.insert(path.clone()))
        .collect();

    let mut lines = vec![Line::from(vec![
        Span::styled("Modified Files", Style::default().fg(TEXT_DIM)),
        Span::styled(" ─────────", Style::default().fg(BORDER_DIM)),
    ])];

    if modified_files.is_empty() {
        lines.push(Line::from(Span::styled(
            "None",
            Style::default().fg(TEXT_MUTED),
        )));
    } else {
        for file in modified_files.iter().take(3) {
            let display = if file.len() > 20 {
                format!("...{}", &file[file.len() - 17..])
            } else {
                file.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("● ", Style::default().fg(ACCENT_GREEN)),
                Span::styled(display, Style::default().fg(TEXT_DIM)),
            ]));
        }
        if modified_files.len() > 3 {
            lines.push(Line::from(Span::styled(
                format!("  +{} more", modified_files.len() - 3),
                Style::default().fg(TEXT_MUTED),
            )));
        }
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Draw help hints at bottom of sidebar
fn draw_help_hints(f: &mut Frame, area: Rect) {
    let lines = vec![Line::from(Span::styled(
        "/help · @file context",
        Style::default().fg(TEXT_MUTED),
    ))];

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
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

/// Colorize a line based on embedded markers (Claude Code style)
fn colorize_line(s: &str) -> Line<'static> {
    // AI response lines - clean bullet style like Claude Code
    // "● " is 4 bytes (● is 3 bytes + space)
    if s.starts_with("● ") {
        let content = if s.len() > 4 { &s[4..] } else { "" };
        Line::from(vec![
            Span::styled("● ", Style::default().fg(TEXT_DIM)),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    // AI response continuation (indented)
    } else if s.starts_with("  ") && !s.trim().is_empty() {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(TEXT_PRIMARY),
        ))
    // Tool execution - compact amber style
    } else if s.starts_with("⚡ ") {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT_AMBER),
        ))
    // Diff lines
    } else if s.starts_with("  - ") {
        Line::from(Span::styled(s.to_string(), Style::default().fg(ACCENT_RED)))
    } else if s.starts_with("  + ") {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT_GREEN),
        ))
    // File path in diff
    } else if s.starts_with("  📝 ") {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(ACCENT_AMBER),
        ))
    // Shell output
    } else if s.starts_with("  ") {
        Line::from(Span::styled(s.to_string(), Style::default().fg(TEXT_DIM)))
    // User input - shell command style
    } else if s.starts_with("> ") {
        let content = &s[2..];
        Line::from(vec![
            Span::styled("> ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                content.to_string(),
                Style::default()
                    .fg(TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    // System/muted messages
    } else if s.starts_with("? ") {
        Line::from(Span::styled(s.to_string(), Style::default().fg(TEXT_MUTED)))
    } else {
        Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(TEXT_PRIMARY),
        ))
    }
}

/// Render a single command block to plain strings (Claude Code style)
fn render_block_to_strings(
    block: &CommandBlock,
    width: usize,
    lines: &mut Vec<String>,
    animation_frame: usize,
) {
    match &block.block_type {
        BlockType::SystemMessage => {
            // System messages - muted, with ? prefix
            let output = block.output.get_text();
            for line in output.lines() {
                let wrapped = wrap(line, width.saturating_sub(2));
                for wrapped_line in wrapped {
                    lines.push(format!("? {}", wrapped_line));
                }
            }
        }

        BlockType::ShellCommand => {
            // User shell command - "> command" style
            let mut header = format!("> {}", block.input);

            if block.is_running() {
                let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
                header.push_str(&format!("  {}", spinner));
            } else if let Some(code) = block.exit_code {
                if code != 0 {
                    header.push_str(&format!(" ✗ {}", code));
                }
            }

            lines.push(header);

            // Shell output - indented
            let output = block.output.get_text();
            if !output.is_empty() {
                for line in output.lines().take(30) {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for wrapped_line in wrapped {
                        lines.push(format!("  {}", wrapped_line));
                    }
                }
                if output.lines().count() > 30 {
                    lines.push("  ... [truncated]".to_string());
                }
            }
        }

        BlockType::AiQuery => {
            // User query - "> query" style (same as shell for consistency)
            let mut header = format!("> {}", block.input);

            if block.is_running() {
                let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
                header.push_str(&format!("  {}", spinner));
            }

            lines.push(header);

            // Render child blocks (tools and reasoning) in order
            if !block.children.is_empty() {
                for child in &block.children {
                    match &child.block_type {
                        BlockType::AiToolExecution { .. } => {
                            render_tool_strings(child, width, lines, animation_frame);
                        }
                        BlockType::AiReasoning => {
                            render_reasoning_strings(child, width, lines);
                        }
                        _ => {}
                    }
                }
            }

            // Render final AI response - bullet point style like Claude Code
            let output = block.output.get_text();
            if !output.is_empty() && !block.is_running() {
                let output_lines: Vec<&str> = output.lines().collect();
                for (i, line) in output_lines.iter().enumerate() {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for (j, wrapped_line) in wrapped.iter().enumerate() {
                        if i == 0 && j == 0 {
                            // First line gets bullet
                            lines.push(format!("● {}", wrapped_line));
                        } else {
                            // Continuation lines are indented
                            lines.push(format!("  {}", wrapped_line));
                        }
                    }
                }
            } else if block.is_running() && output.is_empty() && block.children.is_empty() {
                let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
                lines.push(format!("● {} ...", spinner));
            }
        }

        BlockType::AiReasoning => {
            render_reasoning_strings(block, width, lines);
        }

        BlockType::AiToolExecution { .. } => {
            render_tool_strings(block, width, lines, animation_frame);
        }

        BlockType::Orchestration => {
            let mut header = format!("> orchestrate {}", block.input);

            if block.is_running() {
                let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
                header.push_str(&format!("  {}", spinner));
            }

            lines.push(header);

            let output = block.output.get_text();
            if !output.is_empty() {
                for line in output.lines() {
                    let wrapped = wrap(line, width.saturating_sub(4));
                    for wrapped_line in wrapped {
                        lines.push(format!("  {}", wrapped_line));
                    }
                }
            }
        }
    }
}

/// Render tool execution block strings (compact style)
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

    // Compact tool header
    let mut header = format!("⚡ {}", tool_name);

    // Add brief description if present
    if !block.input.is_empty() {
        let desc = if block.input.chars().count() > 40 {
            // Safe truncation for UTF-8
            let truncated: String = block.input.chars().take(37).collect();
            format!("{}...", truncated)
        } else {
            block.input.clone()
        };
        header.push_str(&format!(" {}", desc));
    }

    if block.is_running() {
        let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
        header.push_str(&format!(" {}", spinner));
    } else if let Some(exit_code) = block.exit_code {
        if exit_code == 0 {
            header.push_str(" ✓");
        } else {
            header.push_str(" ✗");
        }
    }

    lines.push(header);

    // Show diff if present
    if let Some(diff) = &block.diff {
        render_diff_strings(diff, width, lines);
    }
    // Otherwise show minimal output (or nothing for clean look)
}

/// Render AI reasoning text (inline between tools)
fn render_reasoning_strings(block: &CommandBlock, width: usize, lines: &mut Vec<String>) {
    let output = block.output.get_text();
    if output.is_empty() {
        return;
    }

    // Render reasoning as bullet points too
    let output_lines: Vec<&str> = output.lines().collect();
    for (i, line) in output_lines.iter().enumerate() {
        let wrapped = wrap(line, width.saturating_sub(4));
        for (j, wrapped_line) in wrapped.iter().enumerate() {
            if i == 0 && j == 0 {
                lines.push(format!("● {}", wrapped_line));
            } else {
                lines.push(format!("  {}", wrapped_line));
            }
        }
    }
}

/// Render a file diff with color-coded additions and deletions (compact)
fn render_diff_strings(diff: &FileDiff, width: usize, lines: &mut Vec<String>) {
    lines.push(format!("  📝 {}", diff.path));

    let text_diff = TextDiff::from_lines(&diff.old_content, &diff.new_content);

    let inner_width = width.saturating_sub(6);
    let mut change_count = 0;

    for change in text_diff.iter_all_changes() {
        if change_count >= 10 {
            break;
        }

        let line_content = change.value().trim_end();

        let display_content = if line_content.chars().count() > inner_width {
            // Safe truncation for UTF-8
            let truncated: String = line_content
                .chars()
                .take(inner_width.saturating_sub(3))
                .collect();
            format!("{}...", truncated)
        } else {
            line_content.to_string()
        };

        match change.tag() {
            ChangeTag::Delete => {
                lines.push(format!("  - {}", display_content));
                change_count += 1;
            }
            ChangeTag::Insert => {
                lines.push(format!("  + {}", display_content));
                change_count += 1;
            }
            ChangeTag::Equal => {}
        }
    }

    let total_changes = text_diff
        .iter_all_changes()
        .filter(|c| c.tag() != ChangeTag::Equal)
        .count();

    if total_changes > 10 {
        lines.push(format!("  ... ({} more changes)", total_changes - 10));
    }
}

/// Draw the input area at the bottom (Claude Code style with word wrap)
fn draw_input(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    use ratatui::widgets::Wrap;

    // No border, just a clean separator line
    let separator = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER_DIM));
    f.render_widget(separator, area);

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(1),
    };

    let input_color = TEXT_PRIMARY;

    // Build the full input text with prompt and cursor
    let (before_cursor, after_cursor) = app.input.split_at(app.cursor_pos.min(app.input.len()));

    // Cursor character (blinking) - must be String for consistent type
    let cursor_char: String = if app.animation_frame % 20 < 10 {
        if after_cursor.is_empty() {
            "█".to_string()
        } else {
            // Safe first char extraction
            after_cursor
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "█".to_string())
        }
    } else {
        if after_cursor.is_empty() {
            " ".to_string()
        } else {
            after_cursor
                .chars()
                .next()
                .map(|c| c.to_string())
                .unwrap_or_else(|| " ".to_string())
        }
    };

    // Get rest after cursor (skip first char safely)
    let after_cursor_rest: String = after_cursor.chars().skip(1).collect();

    // Build full input string for manual wrapping
    let full_input = format!("> {}", app.input);
    
    // Calculate wrap width, accounting for send hint space if input is not empty
    let hint_space = if app.input.is_empty() { 0 } else { 8 }; // "↵ send" + padding
    let wrap_width = inner.width.saturating_sub(1 + hint_space) as usize; // leave margin + hint space
    let wrap_width = wrap_width.max(5);

    // Manually wrap the text into lines
    let wrapped_lines: Vec<String> = if wrap_width > 0 {
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut char_count = 0;

        for ch in full_input.chars() {
            current_line.push(ch);
            char_count += 1;
            if char_count >= wrap_width {
                lines.push(current_line.clone());
                current_line.clear();
                char_count = 0;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
        if lines.is_empty() {
            lines.push("> ".to_string());
        }
        lines
    } else {
        vec![full_input]
    };

    // Now build styled lines with cursor in the right position
    let cursor_pos_in_full = app.cursor_pos + 2; // +2 for "> "
    let mut styled_lines: Vec<Line> = Vec::new();
    let mut chars_processed = 0;

    for line_str in &wrapped_lines {
        let line_start = chars_processed;
        let line_end = chars_processed + line_str.chars().count();

        // Check if cursor is on this line
        if cursor_pos_in_full >= line_start && cursor_pos_in_full <= line_end {
            // Cursor is on this line - build with cursor highlight
            let cursor_offset = cursor_pos_in_full - line_start;
            let before: String = line_str.chars().take(cursor_offset).collect();
            let at_cursor: String = line_str.chars().skip(cursor_offset).take(1).collect();
            let after: String = line_str.chars().skip(cursor_offset + 1).collect();

            let cursor_display = if at_cursor.is_empty() {
                cursor_char.clone()
            } else {
                at_cursor
            };

            // Style the "> " prompt differently
            let mut spans = if line_start == 0 && cursor_offset > 2 {
                vec![
                    Span::styled("> ", Style::default().fg(TEXT_DIM)),
                    Span::styled(
                        line_str
                            .chars()
                            .skip(2)
                            .take(cursor_offset - 2)
                            .collect::<String>(),
                        Style::default().fg(input_color),
                    ),
                ]
            } else if line_start == 0 {
                // Cursor is in or at the prompt
                vec![Span::styled(before.clone(), Style::default().fg(TEXT_DIM))]
            } else {
                vec![Span::styled(
                    before.clone(),
                    Style::default().fg(input_color),
                )]
            };

            spans.push(Span::styled(
                cursor_display,
                Style::default()
                    .fg(input_color)
                    .add_modifier(Modifier::REVERSED),
            ));
            if !after.is_empty() {
                spans.push(Span::styled(after, Style::default().fg(input_color)));
            }

            styled_lines.push(Line::from(spans));
        } else {
            // No cursor on this line
            if line_start == 0 {
                // First line with prompt
                let rest: String = line_str.chars().skip(2).collect();
                styled_lines.push(Line::from(vec![
                    Span::styled("> ", Style::default().fg(TEXT_DIM)),
                    Span::styled(rest, Style::default().fg(input_color)),
                ]));
            } else {
                styled_lines.push(Line::from(Span::styled(
                    line_str.clone(),
                    Style::default().fg(input_color),
                )));
            }
        }

        chars_processed = line_end;
    }

    let input_paragraph = Paragraph::new(styled_lines);
    f.render_widget(input_paragraph, inner);

    // Show hint on the right side of first line if there's input
    if !app.input.is_empty() {
        let hint_text = "↵ send";
        let hint_x = inner.x + inner.width.saturating_sub(hint_text.len() as u16 + 1);
        let hint_area = Rect {
            x: hint_x,
            y: inner.y,
            width: hint_text.len() as u16 + 1,
            height: 1,
        };
        let hint = Paragraph::new(Span::styled(hint_text, Style::default().fg(TEXT_MUTED)));
        f.render_widget(hint, hint_area);
    }

    // Show helper text below input if empty
    if app.input.is_empty() {
        let helper_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        };
        let helper = Paragraph::new(Line::from(Span::styled(
            "? for shortcuts",
            Style::default().fg(TEXT_MUTED),
        )));
        f.render_widget(helper, helper_area);
    }
}

/// Draw the file picker dropup menu above the input area
fn draw_file_picker(f: &mut Frame, app: &ShellTuiApp, main_area: Rect) {
    let filtered_entries = app.file_picker.filtered_entries();

    if filtered_entries.is_empty() && app.file_picker.filter.is_empty() {
        return;
    }

    // Calculate dimensions
    let max_entries = 10;
    let entry_count = filtered_entries.len().min(max_entries);
    let height = (entry_count + 3) as u16; // +3 for border, title, and filter line

    // Find max entry width for sizing
    let max_name_width = filtered_entries
        .iter()
        .map(|e| e.name.len() + 8) // Icon + size space
        .max()
        .unwrap_or(20);
    let width = (max_name_width as u16 + 4)
        .min(main_area.width.saturating_sub(4))
        .max(30);

    // Position dropup above input area
    let x = main_area.x + 2;
    let y = main_area.height.saturating_sub(height + 3); // +3 for input area

    let popup_area = Rect {
        x,
        y,
        width,
        height,
    };

    // Build title with path
    let title = if app.file_picker.current_dir.is_empty() {
        " Files ".to_string()
    } else {
        format!(" {} ", app.file_picker.current_dir)
    };

    let popup_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_DARK));

    let inner = popup_block.inner(popup_area);

    // Clear the area and draw block
    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(popup_block, popup_area);

    // Draw filter input at top of picker
    let filter_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };

    let filter_text = if app.file_picker.filter.is_empty() {
        "Type to filter...".to_string()
    } else {
        app.file_picker.filter.clone()
    };

    let filter_style = if app.file_picker.filter.is_empty() {
        Style::default().fg(TEXT_MUTED)
    } else {
        Style::default().fg(ACCENT_CYAN)
    };

    let filter_para = Paragraph::new(Line::from(vec![
        Span::styled("🔍 ", Style::default().fg(ACCENT_CYAN)),
        Span::styled(filter_text, filter_style),
    ]));
    f.render_widget(filter_para, filter_area);

    // Draw entries list
    let list_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };

    if filtered_entries.is_empty() {
        let no_match = Paragraph::new(Line::from(Span::styled(
            "No matches",
            Style::default().fg(TEXT_MUTED),
        )));
        f.render_widget(no_match, list_area);
        return;
    }

    let items: Vec<ListItem> = filtered_entries
        .iter()
        .enumerate()
        .take(max_entries)
        .map(|(i, entry)| {
            let style = if i == app.file_picker.selected {
                Style::default()
                    .fg(BG_DARK)
                    .bg(ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(TEXT_PRIMARY)
            };

            // Icon based on type
            let icon = if entry.is_dir {
                "📁 "
            } else {
                // File icon based on extension
                let ext = entry.name.rsplit('.').next().unwrap_or("");
                match ext {
                    "rs" => "🦀 ",
                    "js" | "ts" | "jsx" | "tsx" => "📜 ",
                    "py" => "🐍 ",
                    "json" | "toml" | "yaml" | "yml" => "⚙️ ",
                    "md" => "📝 ",
                    "html" | "css" => "🌐 ",
                    _ => "📄 ",
                }
            };

            // Size display for files
            let size_str = if let Some(size) = entry.size {
                format!(" {}", FilePicker::format_size(size))
            } else {
                String::new()
            };

            // Truncate name if needed (safe for UTF-8)
            let max_name_len =
                (list_area.width as usize).saturating_sub(icon.len() + size_str.len() + 2);
            let display_name = if entry.name.chars().count() > max_name_len {
                let truncated: String = entry
                    .name
                    .chars()
                    .take(max_name_len.saturating_sub(3))
                    .collect();
                format!("{}...", truncated)
            } else {
                entry.name.clone()
            };

            ListItem::new(Line::from(vec![
                Span::raw(icon),
                Span::styled(display_name, style),
                Span::styled(size_str, Style::default().fg(TEXT_DIM)),
            ]))
            .style(style)
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, list_area);

    // Show scroll indicator if more entries
    if filtered_entries.len() > max_entries {
        let more_text = format!("↓ {} more", filtered_entries.len() - max_entries);
        let more_area = Rect {
            x: popup_area.x + popup_area.width - more_text.len() as u16 - 2,
            y: popup_area.y + popup_area.height - 1,
            width: more_text.len() as u16 + 1,
            height: 1,
        };
        let more_para = Paragraph::new(Span::styled(more_text, Style::default().fg(TEXT_MUTED)));
        f.render_widget(more_para, more_area);
    }
}

/// Draw the autocomplete popup above the input area
fn draw_autocomplete(f: &mut Frame, app: &ShellTuiApp, input_area: Rect) {
    let suggestions = &app.autocomplete.suggestions;
    let selected = app.autocomplete.selected;

    if suggestions.is_empty() {
        return;
    }

    let max_width = suggestions.iter().map(|s| s.len()).max().unwrap_or(10) + 4;
    let width = (max_width as u16)
        .min(input_area.width.saturating_sub(4))
        .max(15);
    let height = (suggestions.len() as u16 + 2).min(12);

    let x = input_area.x + 2;
    let y = input_area.y.saturating_sub(height);

    let popup_area = Rect {
        x,
        y,
        width,
        height,
    };

    let popup_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT_CYAN))
        .style(Style::default().bg(BG_DARK));

    let inner = popup_block.inner(popup_area);

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(popup_block, popup_area);

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
