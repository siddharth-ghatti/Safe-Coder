use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use textwrap::wrap;

use super::app::{App, FocusPanel};
use super::banner;
use super::messages::MessageType;

// 🌃 CYBERPUNK NEON COLOR SCHEME 🌃
const NEON_CYAN: Color = Color::Rgb(0, 255, 255);           // Electric cyan
const NEON_MAGENTA: Color = Color::Rgb(255, 0, 255);        // Hot magenta
const NEON_PURPLE: Color = Color::Rgb(160, 32, 240);        // Deep purple
const NEON_PINK: Color = Color::Rgb(255, 20, 147);          // Hot pink
const NEON_GREEN: Color = Color::Rgb(57, 255, 20);          // Neon green
const NEON_BLUE: Color = Color::Rgb(125, 249, 255);         // Electric blue
const CYBER_RED: Color = Color::Rgb(255, 0, 85);            // Cyberpunk red
const DARK_BG: Color = Color::Rgb(10, 0, 20);               // Dark purple-black
const GRID_COLOR: Color = Color::Rgb(80, 0, 120);           // Grid lines

// Mapped colors
const PRIMARY_COLOR: Color = NEON_CYAN;                      // Primary actions
const SECONDARY_COLOR: Color = NEON_MAGENTA;                 // Secondary elements
const SUCCESS_COLOR: Color = NEON_GREEN;                     // Success states
const ERROR_COLOR: Color = CYBER_RED;                        // Errors
const WARNING_COLOR: Color = NEON_PINK;                      // Warnings/Processing
const BG_COLOR: Color = Color::Black;                        // Pure black
const TEXT_COLOR: Color = NEON_CYAN;                         // Cyan text
const BORDER_COLOR: Color = NEON_PURPLE;                     // Purple borders
const DIM_TEXT: Color = NEON_BLUE;                           // Dimmed cyan

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Create the main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(9),  // Header with ASCII art
            Constraint::Min(0),     // Main content
            Constraint::Length(4),  // Input with processing status
            Constraint::Length(1),  // Footer
        ])
        .split(size);

    // Draw header
    draw_header(f, app, chunks[0]);

    // Split main content area
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70), // Chat area
            Constraint::Percentage(30), // Side panel
        ])
        .split(chunks[1]);

    // Draw chat area
    draw_chat(f, app, main_chunks[0]);

    // Split side panel
    let side_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // VM Status
            Constraint::Min(0),     // Tools
        ])
        .split(main_chunks[1]);

    // Draw VM status
    draw_vm_status(f, app, side_chunks[0]);

    // Draw tools
    draw_tools(f, app, side_chunks[1]);

    // Draw input
    draw_input(f, app, chunks[2]);

    // Draw footer
    draw_footer(f, app, chunks[3]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // 🌃 CYBERPUNK NEON BANNER with alternating colors 🌃
    let banner_lines: Vec<Line> = banner::BANNER_SMALL
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // Alternate between cyan and magenta for that cyberpunk vibe
            let color = match i {
                0 => NEON_CYAN,
                1 => NEON_MAGENTA,
                2 => NEON_CYAN,
                3 => NEON_PINK,
                4 => NEON_MAGENTA,
                _ => NEON_PURPLE,
            };
            Line::from(Span::styled(
                line,
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::RAPID_BLINK) // Glitch effect!
            ))
        })
        .collect();

    // Glowing border effect
    let border_color = if app.animation_frame % 20 < 10 {
        NEON_CYAN
    } else {
        NEON_MAGENTA
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG_COLOR));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Render banner
    let banner_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 6,
    };

    let banner_paragraph = Paragraph::new(banner_lines)
        .alignment(Alignment::Center);
    f.render_widget(banner_paragraph, banner_area);

    // Cyberpunk project info with glitch effect
    let info_area = Rect {
        x: inner.x,
        y: inner.y + 6,
        width: inner.width,
        height: 1,
    };

    let glitch = if app.animation_frame % 50 == 0 { "█" } else { "" };
    let info = Line::from(vec![
        Span::styled("  ▶ ", Style::default().fg(NEON_PINK)),
        Span::styled("PROJECT:", Style::default().fg(NEON_PURPLE).add_modifier(Modifier::BOLD)),
        Span::styled(" ", Style::default()),
        Span::styled(&app.project_path, Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(glitch, Style::default().fg(NEON_MAGENTA)),
    ]);

    f.render_widget(Paragraph::new(info).alignment(Alignment::Center), info_area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Chat;

    // Pulsing border effect
    let border_color = if is_focused {
        if app.animation_frame % 30 < 15 { NEON_CYAN } else { NEON_MAGENTA }
    } else {
        BORDER_COLOR
    };

    let block = Block::default()
        .title(vec![
            Span::styled("▶▶ ", Style::default().fg(NEON_PINK)),
            Span::styled("NEURAL LINK", Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" ◀◀", Style::default().fg(NEON_PINK)),
        ])
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG_COLOR));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Calculate visible messages
    let max_lines = inner_area.height as usize;
    let mut items = Vec::new();
    let mut line_count = 0;

    for msg in app.messages.iter().rev().skip(app.scroll_offset) {
        if line_count >= max_lines {
            break;
        }

        let (prefix, style, icon) = match msg.message_type {
            MessageType::User => ("You", Style::default().fg(PRIMARY_COLOR), "👤"),
            MessageType::Assistant => ("Assistant", Style::default().fg(SUCCESS_COLOR), "🤖"),
            MessageType::System => ("System", Style::default().fg(WARNING_COLOR), "ℹ️"),
            MessageType::Error => ("Error", Style::default().fg(ERROR_COLOR), "❌"),
            MessageType::Tool => ("Tool", Style::default().fg(SECONDARY_COLOR), "🔧"),
        };

        let time = msg.timestamp.format("%H:%M:%S");

        // Wrap content
        let width = (inner_area.width.saturating_sub(4)) as usize;
        let wrapped = wrap(&msg.content, width);

        for (i, line) in wrapped.iter().enumerate() {
            if line_count >= max_lines {
                break;
            }

            if i == 0 {
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", icon), style),
                    Span::styled(format!("[{}] ", time), Style::default().fg(DIM_TEXT)),
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                    Span::raw(": "),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_COLOR)),
                ])));
            } else {
                items.push(ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_COLOR)),
                ])));
            }
            line_count += 1;
        }

        // Add spacing
        if line_count < max_lines {
            items.push(ListItem::new(Line::from("")));
            line_count += 1;
        }
    }

    items.reverse();

    let list = List::new(items);
    f.render_widget(list, inner_area);
}

fn draw_vm_status(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Status;
    let border_color = if is_focused {
        if app.animation_frame % 30 < 15 { NEON_CYAN } else { NEON_MAGENTA }
    } else {
        BORDER_COLOR
    };

    let status_icon = if app.vm_status.running { "◉" } else { "◯" };
    let status_text = if app.vm_status.running { "ONLINE" } else { "OFFLINE" };
    let status_color = if app.vm_status.running { NEON_GREEN } else { CYBER_RED };

    let pulse = if app.vm_status.running && app.animation_frame % 20 < 10 { "▓" } else { "▒" };

    let content = vec![
        Line::from(vec![
            Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
            Span::styled("STATUS: ", Style::default().fg(NEON_PURPLE).add_modifier(Modifier::BOLD)),
            Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {}", pulse), Style::default().fg(status_color)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("▶ ", Style::default().fg(NEON_PINK)),
            Span::styled("UPTIME: ", Style::default().fg(NEON_BLUE)),
            Span::styled(&app.vm_status.uptime, Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("▶ ", Style::default().fg(NEON_PINK)),
            Span::styled("MEMORY: ", Style::default().fg(NEON_BLUE)),
            Span::styled(format!("{} MB", app.vm_status.memory_mb), Style::default().fg(NEON_MAGENTA).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("▶ ", Style::default().fg(NEON_PINK)),
            Span::styled("vCPUs: ", Style::default().fg(NEON_BLUE)),
            Span::styled(format!("{}", app.vm_status.vcpus), Style::default().fg(NEON_PURPLE).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let block = Block::default()
        .title(vec![
            Span::styled("◢◤ ", Style::default().fg(NEON_PINK)),
            Span::styled("SYSTEM STATUS", Style::default().fg(NEON_CYAN).add_modifier(Modifier::BOLD)),
            Span::styled(" ◢◤", Style::default().fg(NEON_PINK)),
        ])
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG_COLOR));

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

fn draw_tools(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Tools;
    let border_color = if is_focused {
        if app.animation_frame % 30 < 15 { NEON_CYAN } else { NEON_MAGENTA }
    } else {
        BORDER_COLOR
    };

    let block = Block::default()
        .title(vec![
            Span::styled("◢◤ ", Style::default().fg(NEON_PINK)),
            Span::styled("TOOL EXECUTION", Style::default().fg(NEON_MAGENTA).add_modifier(Modifier::BOLD)),
            Span::styled(" ◢◤", Style::default().fg(NEON_PINK)),
        ])
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG_COLOR));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = app
        .tool_executions
        .iter()
        .rev()
        .take(inner_area.height as usize)
        .map(|tool| {
            let (icon, color) = match tool.status {
                super::messages::ToolStatus::Running => ("◉", NEON_PINK),
                super::messages::ToolStatus::Success => ("◉", NEON_GREEN),
                super::messages::ToolStatus::Failed => ("◉", CYBER_RED),
            };

            ListItem::new(Line::from(vec![
                Span::styled("▶ ", Style::default().fg(NEON_CYAN)),
                Span::styled(format!("{} ", icon), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::styled(&tool.tool_name, Style::default().fg(NEON_MAGENTA).add_modifier(Modifier::BOLD)),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner_area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    // Intense border pulsing when processing
    let border_color = if app.is_thinking {
        if app.animation_frame % 10 < 5 { NEON_PINK } else { NEON_MAGENTA }
    } else {
        if app.animation_frame % 40 < 20 { NEON_CYAN } else { NEON_PURPLE }
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(BG_COLOR));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into input line and processing status
    let input_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Input line
            Constraint::Length(1),  // Processing status
        ])
        .split(inner);

    // Draw input line with cyberpunk flair
    let input_text = if app.is_thinking {
        vec![
            Span::styled("  ", Style::default()),
        ]
    } else {
        let cursor_color = if app.animation_frame % 20 < 10 { NEON_CYAN } else { NEON_MAGENTA };
        vec![
            Span::styled("  >>", Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
            Span::styled(&app.input, Style::default().fg(NEON_CYAN)),
            Span::styled("█", Style::default().fg(cursor_color).add_modifier(Modifier::RAPID_BLINK)),
        ]
    };

    f.render_widget(Paragraph::new(Line::from(input_text)), input_chunks[0]);

    // Draw INTENSE cyberpunk processing status
    if app.is_thinking {
        let frames = banner::get_processing_frames();
        let frame = frames[app.animation_frame % frames.len()];
        let dots = ".".repeat((app.animation_frame / 10) % 4);

        let processing_text = if app.processing_message.is_empty() {
            format!("PROCESSING{}", dots)
        } else {
            format!("{}{}", app.processing_message.to_uppercase(), dots)
        };

        let flash_color = if app.animation_frame % 6 < 3 { NEON_PINK } else { NEON_MAGENTA };

        let status_line = vec![
            Span::styled("  ▶▶ ", Style::default().fg(NEON_CYAN)),
            Span::styled(frame, Style::default().fg(flash_color).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
            Span::styled(processing_text, Style::default().fg(NEON_PINK).add_modifier(Modifier::BOLD)),
            Span::styled(" ◀◀", Style::default().fg(NEON_CYAN)),
        ];

        f.render_widget(Paragraph::new(Line::from(status_line)), input_chunks[1]);
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let footer_text = vec![
        Span::styled("^C", Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD)),
        Span::styled(" Exit ", Style::default().fg(TEXT_COLOR)),
        Span::styled("│", Style::default().fg(BORDER_COLOR)),
        Span::styled(" ↑↓", Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD)),
        Span::styled(" Scroll ", Style::default().fg(TEXT_COLOR)),
        Span::styled("│", Style::default().fg(BORDER_COLOR)),
        Span::styled(" Tab", Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD)),
        Span::styled(" Switch Panel ", Style::default().fg(TEXT_COLOR)),
        Span::styled("│", Style::default().fg(BORDER_COLOR)),
        Span::styled(" Status: ", Style::default().fg(TEXT_COLOR)),
        Span::styled(&app.status, Style::default().fg(SUCCESS_COLOR)),
    ];

    let paragraph = Paragraph::new(Line::from(footer_text))
        .style(Style::default().bg(BG_COLOR).fg(TEXT_COLOR))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}
