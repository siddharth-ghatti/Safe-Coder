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

use super::shell_app::{BlockType, CommandBlock, FileDiff, InputMode, PermissionMode, ShellTuiApp};

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
}

/// Draw the main content area (left side)
fn draw_main_content(f: &mut Frame, app: &mut ShellTuiApp, area: Rect) {
    // Vertical layout: blocks (top), input (bottom)
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Command blocks
            Constraint::Length(3), // Input area
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
    // Count files that were edited in recent tool calls
    let modified_files: Vec<String> = app
        .blocks
        .iter()
        .flat_map(|b| &b.children)
        .filter_map(|child| child.diff.as_ref().map(|d| d.path.clone()))
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
        "ctrl+c quit · @ ai",
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

/// Colorize a line based on embedded markers
fn colorize_line(s: &str) -> Line<'static> {
    if s.starts_with("│M ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_MAGENTA)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│G ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_GREEN)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│A ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_DIM)),
        ])
    } else if s.starts_with("│R ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_RED)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("│D-") {
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!(" - {}", content), Style::default().fg(ACCENT_RED)),
        ])
    } else if s.starts_with("│D+") {
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!(" + {}", content), Style::default().fg(ACCENT_GREEN)),
        ])
    } else if s.starts_with("│D ") {
        let content = &s[3..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::styled(format!("   {}", content), Style::default().fg(TEXT_MUTED)),
        ])
    } else if s.starts_with("│T ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(ACCENT_AMBER)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(ACCENT_AMBER)),
        ])
    } else if s.starts_with("│_ ") {
        let content = &s[4..];
        Line::from(vec![
            Span::styled("│", Style::default().fg(TEXT_MUTED)),
            Span::raw(" "),
            Span::styled(content.to_string(), Style::default().fg(TEXT_MUTED)),
        ])
    } else if s.starts_with("> ") {
        let content = &s[2..];
        Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(ACCENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(content.to_string(), Style::default().fg(TEXT_PRIMARY)),
        ])
    } else if s.starts_with("@ ") {
        let content = &s[2..];
        Line::from(vec![
            Span::styled(
                "@ ",
                Style::default()
                    .fg(ACCENT_MAGENTA)
                    .add_modifier(Modifier::BOLD),
            ),
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
            let mut header = format!("@ {}", block.input);

            if block.is_running() {
                let thinking = THINKING_FRAMES[(animation_frame / 3) % THINKING_FRAMES.len()];
                let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
                header.push_str(&format!("  {} thinking {}", thinking, spinner));
            }

            lines.push(header);
            lines.push(String::new());

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
                    lines.push(String::new());
                }
            }

            // Render final AI response
            let output = block.output.get_text();
            if !output.is_empty() && !block.is_running() {
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
                lines.push("│M ...".to_string());
            }
        }

        BlockType::AiReasoning => {
            render_reasoning_strings(block, width, lines);
        }

        BlockType::AiToolExecution { .. } => {
            render_tool_strings(block, width, lines, animation_frame);
        }

        BlockType::Orchestration => {
            let mut header = format!("⚙ orchestrate {}", block.input);

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

    let mut header = format!("⚡ {}", tool_name);

    if !block.input.is_empty() {
        header.push_str(&format!(" {}", block.input));
    }

    if block.is_running() {
        // Animated progress bar
        let progress_width = 8;
        let pos = animation_frame % (progress_width * 2);
        let mut bar = String::new();
        for i in 0..progress_width {
            if (pos < progress_width && i == pos)
                || (pos >= progress_width && i == progress_width * 2 - pos - 1)
            {
                bar.push_str(PROGRESS_CHARS[3]); // Full block
            } else {
                bar.push_str(PROGRESS_CHARS[0]); // Empty
            }
        }
        let spinner = SPINNER_FRAMES[animation_frame % SPINNER_FRAMES.len()];
        header.push_str(&format!(" {} [{}]", spinner, bar));
    } else if let Some(exit_code) = block.exit_code {
        if exit_code == 0 {
            header.push_str(" ✓");
        } else {
            header.push_str(" ✗");
        }
    }

    lines.push(format!("│T {}", header));

    if let Some(diff) = &block.diff {
        render_diff_strings(diff, width, lines);
    } else {
        let output = block.output.get_text();
        if !output.is_empty() {
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
                lines.push(format!(
                    "│_ ... ({} more lines)",
                    output.lines().count() - 5
                ));
            }
        }
    }
}

/// Render AI reasoning text (inline between tools)
fn render_reasoning_strings(block: &CommandBlock, width: usize, lines: &mut Vec<String>) {
    let output = block.output.get_text();
    if output.is_empty() {
        return;
    }

    for line in output.lines() {
        let wrapped = wrap(line, width.saturating_sub(4));
        for wrapped_line in wrapped {
            lines.push(format!("│M {}", wrapped_line));
        }
    }
}

/// Render a file diff with color-coded additions and deletions
fn render_diff_strings(diff: &FileDiff, width: usize, lines: &mut Vec<String>) {
    lines.push(format!("│A 📝 {}", diff.path));

    let text_diff = TextDiff::from_lines(&diff.old_content, &diff.new_content);

    let inner_width = width.saturating_sub(8);
    let mut has_changes = false;
    let mut change_count = 0;

    for change in text_diff.iter_all_changes() {
        if change_count >= 20 {
            break;
        }

        let line_content = change.value().trim_end();

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
            ChangeTag::Equal => {}
        }
    }

    if !has_changes {
        lines.push("│_ (no changes)".to_string());
    } else if text_diff
        .iter_all_changes()
        .filter(|c| c.tag() != ChangeTag::Equal)
        .count()
        > 20
    {
        lines.push("│_ ... (more changes)".to_string());
    }
}

/// Draw the input area at the bottom
fn draw_input(f: &mut Frame, app: &ShellTuiApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER_DIM));

    let inner = block.inner(area);
    f.render_widget(block, area);

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

    let (before_cursor, after_cursor) = app.input.split_at(app.cursor_pos.min(app.input.len()));

    let input_color = match app.input_mode {
        InputMode::AiPrefix => ACCENT_MAGENTA,
        _ => TEXT_PRIMARY,
    };

    spans.push(Span::styled(
        before_cursor.to_string(),
        Style::default().fg(input_color),
    ));

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
