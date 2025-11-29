use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::border,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use textwrap::wrap;

use super::app::{App, FocusPanel};
use super::banner;
use super::messages::MessageType;

// ✨ GEMINI-INSPIRED CLEAN COLOR SCHEME ✨
// A modern, professional palette inspired by Google's Gemini CLI

// Primary palette - Deep blues and teals
const GEMINI_BLUE: Color = Color::Rgb(66, 133, 244);        // Google blue
const GEMINI_TEAL: Color = Color::Rgb(0, 188, 212);         // Bright teal accent
const GEMINI_INDIGO: Color = Color::Rgb(63, 81, 181);       // Indigo for depth

// Secondary palette - Softer accents
const ACCENT_PURPLE: Color = Color::Rgb(149, 117, 205);     // Soft purple
const ACCENT_GREEN: Color = Color::Rgb(129, 199, 132);      // Soft green for success
const ACCENT_AMBER: Color = Color::Rgb(255, 179, 0);        // Warm amber for warnings
const ACCENT_RED: Color = Color::Rgb(239, 83, 80);          // Soft red for errors

// Neutral tones
const TEXT_PRIMARY: Color = Color::Rgb(224, 224, 224);      // Light gray text
const TEXT_SECONDARY: Color = Color::Rgb(158, 158, 158);    // Muted text
const BORDER_NORMAL: Color = Color::Rgb(66, 66, 66);        // Subtle border
const BORDER_FOCUS: Color = Color::Rgb(100, 181, 246);      // Focus highlight
const BG_COLOR: Color = Color::Reset;                       // Terminal default

// Layout constants
const HEADER_HEIGHT: u16 = 8;   // Height for ASCII art banner + project info
const INPUT_HEIGHT: u16 = 3;    // Height for input area with border
const FOOTER_HEIGHT: u16 = 1;   // Height for keyboard hints
const STATUS_HEIGHT: u16 = 7;   // Height for status panel

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Create the main layout - clean and modern
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Min(0),     // Main content (fills remaining space)
            Constraint::Length(INPUT_HEIGHT),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .split(size);

    // Draw header
    draw_header(f, app, chunks[0]);

    // Split main content area
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(72), // Chat area
            Constraint::Percentage(28), // Side panel
        ])
        .split(chunks[1]);

    // Draw chat area
    draw_chat(f, app, main_chunks[0]);

    // Split side panel
    let side_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(STATUS_HEIGHT),
            Constraint::Min(0),     // Tools (fills remaining space)
        ])
        .split(main_chunks[1]);

    // Draw status
    draw_vm_status(f, app, side_chunks[0]);

    // Draw tools
    draw_tools(f, app, side_chunks[1]);

    // Draw input
    draw_input(f, app, chunks[2]);

    // Draw footer
    draw_footer(f, app, chunks[3]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // ✨ Clean, modern banner with gradient effect ✨
    let banner_lines: Vec<Line> = banner::BANNER_SMALL
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // Gradient from bright blue to teal - smooth and professional
            let color = match i {
                0 => GEMINI_BLUE,
                1 => Color::Rgb(50, 150, 230),
                2 => Color::Rgb(35, 165, 220),
                3 => GEMINI_TEAL,
                4 => Color::Rgb(30, 160, 200),
                _ => GEMINI_INDIGO,
            };
            Line::from(Span::styled(
                line,
                Style::default()
                    .fg(color)
                    .add_modifier(Modifier::BOLD)
            ))
        })
        .collect();

    // Subtle, rounded border
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(BORDER_NORMAL))
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

    // Clean project info
    let info_area = Rect {
        x: inner.x,
        y: inner.y + 6,
        width: inner.width,
        height: 1,
    };

    let info = Line::from(vec![
        Span::styled("󰉋 ", Style::default().fg(GEMINI_TEAL)),
        Span::styled(&app.project_path, Style::default().fg(TEXT_SECONDARY)),
    ]);

    f.render_widget(Paragraph::new(info).alignment(Alignment::Center), info_area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Chat;

    // Clean, subtle border - highlight when focused
    let border_color = if is_focused { BORDER_FOCUS } else { BORDER_NORMAL };

    let block = Block::default()
        .title(vec![
            Span::styled(" 💬 ", Style::default().fg(GEMINI_BLUE)),
            Span::styled("Chat", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ])
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
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
            MessageType::User => ("You", Style::default().fg(GEMINI_BLUE), "◆"),
            MessageType::Assistant => ("Assistant", Style::default().fg(ACCENT_GREEN), "●"),
            MessageType::System => ("System", Style::default().fg(ACCENT_AMBER), "◎"),
            MessageType::Error => ("Error", Style::default().fg(ACCENT_RED), "✕"),
            MessageType::Tool => ("Tool", Style::default().fg(ACCENT_PURPLE), "◇"),
            MessageType::Orchestration => ("Orchestrator", Style::default().fg(GEMINI_TEAL), "◈"),
        };

        let time = msg.timestamp.format("%H:%M");

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
                    Span::styled(format!("{} ", time), Style::default().fg(TEXT_SECONDARY)),
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                    Span::styled(": ", Style::default().fg(TEXT_SECONDARY)),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_PRIMARY)),
                ])));
            } else {
                items.push(ListItem::new(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_PRIMARY)),
                ])));
            }
            line_count += 1;
        }

        // Add subtle spacing
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
    let border_color = if is_focused { BORDER_FOCUS } else { BORDER_NORMAL };

    // Show orchestration status if orchestrating, otherwise show VM status
    let content = if app.is_orchestrating || !app.background_tasks.is_empty() {
        let active = app.get_active_tasks_count();
        let completed = app.get_completed_tasks_count();
        let failed = app.get_failed_tasks_count();
        
        let status_icon = if active > 0 { "●" } else { "○" };
        let status_text = if active > 0 { "Active" } else { "Idle" };
        let status_color = if active > 0 { ACCENT_AMBER } else { ACCENT_GREEN };

        vec![
            Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Active: ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(format!("{}", active), Style::default().fg(GEMINI_BLUE).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  Done:   ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(format!("{}", completed), Style::default().fg(ACCENT_GREEN)),
            ]),
            Line::from(vec![
                Span::styled("  Failed: ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(format!("{}", failed), Style::default().fg(if failed > 0 { ACCENT_RED } else { TEXT_SECONDARY })),
            ]),
        ]
    } else {
        let status_icon = if app.vm_status.running { "●" } else { "○" };
        let status_text = if app.vm_status.running { "Online" } else { "Offline" };
        let status_color = if app.vm_status.running { ACCENT_GREEN } else { TEXT_SECONDARY };

        vec![
            Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Uptime: ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(&app.vm_status.uptime, Style::default().fg(TEXT_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled("  Memory: ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(format!("{} MB", app.vm_status.memory_mb), Style::default().fg(TEXT_PRIMARY)),
            ]),
            Line::from(vec![
                Span::styled("  vCPUs:  ", Style::default().fg(TEXT_SECONDARY)),
                Span::styled(format!("{}", app.vm_status.vcpus), Style::default().fg(TEXT_PRIMARY)),
            ]),
        ]
    };

    let title = if app.is_orchestrating || !app.background_tasks.is_empty() {
        vec![
            Span::styled(" ⚙ ", Style::default().fg(GEMINI_TEAL)),
            Span::styled("Workers", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]
    } else {
        vec![
            Span::styled(" ◉ ", Style::default().fg(ACCENT_GREEN)),
            Span::styled("Status", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ]
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG_COLOR));

    let paragraph = Paragraph::new(content).block(block);
    f.render_widget(paragraph, area);
}

fn draw_tools(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Tools;
    let border_color = if is_focused { BORDER_FOCUS } else { BORDER_NORMAL };

    // Show background tasks if orchestrating, otherwise show tool executions
    let (title, items): (Vec<Span>, Vec<ListItem>) = if !app.background_tasks.is_empty() {
        let title = vec![
            Span::styled(" ◇ ", Style::default().fg(ACCENT_PURPLE)),
            Span::styled("Tasks", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ];
        
        let items: Vec<ListItem> = app
            .background_tasks
            .iter()
            .rev()
            .take(area.height.saturating_sub(2) as usize)
            .map(|task| {
                let (icon, color) = match &task.status {
                    super::messages::BackgroundTaskStatus::Pending => ("○", TEXT_SECONDARY),
                    super::messages::BackgroundTaskStatus::Running => ("●", ACCENT_AMBER),
                    super::messages::BackgroundTaskStatus::Completed => ("✓", ACCENT_GREEN),
                    super::messages::BackgroundTaskStatus::Failed(_) => ("✕", ACCENT_RED),
                };

                // Truncate description if too long
                let desc = if task.description.len() > 20 {
                    format!("{}...", &task.description[..17])
                } else {
                    task.description.clone()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                    Span::styled(desc, Style::default().fg(TEXT_PRIMARY)),
                ]))
            })
            .collect();
        
        (title, items)
    } else {
        let title = vec![
            Span::styled(" 🔧 ", Style::default().fg(ACCENT_PURPLE)),
            Span::styled("Tools", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
            Span::styled(" ", Style::default()),
        ];
        
        let items: Vec<ListItem> = app
            .tool_executions
            .iter()
            .rev()
            .take(area.height.saturating_sub(2) as usize)
            .map(|tool| {
                let (icon, color) = match tool.status {
                    super::messages::ToolStatus::Running => ("●", ACCENT_AMBER),
                    super::messages::ToolStatus::Success => ("✓", ACCENT_GREEN),
                    super::messages::ToolStatus::Failed => ("✕", ACCENT_RED),
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!(" {} ", icon), Style::default().fg(color)),
                    Span::styled(&tool.tool_name, Style::default().fg(TEXT_PRIMARY)),
                ]))
            })
            .collect();
        
        (title, items)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG_COLOR));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let list = List::new(items);
    f.render_widget(list, inner_area);
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    // Subtle border that highlights when thinking
    let border_color = if app.is_thinking {
        ACCENT_AMBER
    } else {
        BORDER_FOCUS
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(BG_COLOR));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Draw input line with clean, minimal style
    let input_text = if app.is_thinking {
        // Show processing state with smooth spinner
        let frames = banner::get_processing_frames();
        let frame = frames[app.animation_frame % frames.len()];
        let dots = ".".repeat((app.animation_frame / 15) % 4);

        let processing_text = if app.processing_message.is_empty() {
            format!("Thinking{}", dots)
        } else {
            format!("{}{}", app.processing_message, dots)
        };

        vec![
            Span::styled("  ", Style::default()),
            Span::styled(frame, Style::default().fg(ACCENT_AMBER)),
            Span::styled(" ", Style::default()),
            Span::styled(processing_text, Style::default().fg(ACCENT_AMBER).add_modifier(Modifier::ITALIC)),
        ]
    } else {
        let cursor = if app.animation_frame % 20 < 10 { "▌" } else { " " };
        vec![
            Span::styled("  ❯ ", Style::default().fg(GEMINI_BLUE).add_modifier(Modifier::BOLD)),
            Span::styled(&app.input, Style::default().fg(TEXT_PRIMARY)),
            Span::styled(cursor, Style::default().fg(GEMINI_BLUE)),
        ]
    };

    f.render_widget(Paragraph::new(Line::from(input_text)), inner);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let footer_text = vec![
        Span::styled("^C", Style::default().fg(GEMINI_BLUE).add_modifier(Modifier::BOLD)),
        Span::styled(" Exit", Style::default().fg(TEXT_SECONDARY)),
        Span::styled("  │  ", Style::default().fg(BORDER_NORMAL)),
        Span::styled("/orch", Style::default().fg(GEMINI_TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(" Orchestrate", Style::default().fg(TEXT_SECONDARY)),
        Span::styled("  │  ", Style::default().fg(BORDER_NORMAL)),
        Span::styled("↑↓", Style::default().fg(GEMINI_BLUE).add_modifier(Modifier::BOLD)),
        Span::styled(" Scroll", Style::default().fg(TEXT_SECONDARY)),
        Span::styled("  │  ", Style::default().fg(BORDER_NORMAL)),
        Span::styled("Tab", Style::default().fg(GEMINI_BLUE).add_modifier(Modifier::BOLD)),
        Span::styled(" Focus", Style::default().fg(TEXT_SECONDARY)),
        Span::styled("  │  ", Style::default().fg(BORDER_NORMAL)),
        Span::styled(&app.status, Style::default().fg(ACCENT_GREEN)),
    ];

    let paragraph = Paragraph::new(Line::from(footer_text))
        .style(Style::default().bg(BG_COLOR).fg(TEXT_SECONDARY))
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}
