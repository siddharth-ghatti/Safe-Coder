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

// CRUSH-INSPIRED DARK THEME
// Clean, minimal design with muted colors and purple/pink accents

// Primary accents - purple/magenta tones
const ACCENT_PURPLE: Color = Color::Rgb(180, 120, 200); // Soft purple
const ACCENT_MAGENTA: Color = Color::Rgb(200, 100, 180); // Pink/magenta
const ACCENT_PINK: Color = Color::Rgb(220, 130, 180); // Lighter pink

// Status colors
const STATUS_GREEN: Color = Color::Rgb(120, 200, 140); // Muted green
const STATUS_AMBER: Color = Color::Rgb(220, 180, 100); // Warm amber
const STATUS_RED: Color = Color::Rgb(220, 100, 100); // Muted red

// Neutral tones - clean grays
const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 220); // Almost white
const TEXT_SECONDARY: Color = Color::Rgb(140, 140, 140); // Muted gray
const TEXT_DIM: Color = Color::Rgb(100, 100, 100); // Dimmed text
const BORDER_COLOR: Color = Color::Rgb(60, 60, 65); // Subtle border
const SIDEBAR_BG: Color = Color::Rgb(30, 30, 35); // Slightly lighter bg for sidebar
const BG_COLOR: Color = Color::Reset; // Terminal default

// Layout constants - optimized for Crush-like appearance
const HEADER_HEIGHT: u16 = 7; // ASCII art banner
const INPUT_HEIGHT: u16 = 3; // Input area
const FOOTER_HEIGHT: u16 = 1; // Keyboard hints bar
const SIDEBAR_WIDTH: u16 = 32; // Right sidebar width

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Main vertical layout
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Min(0), // Main content
            Constraint::Length(INPUT_HEIGHT),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .split(size);

    // Draw header with ASCII art
    draw_header(f, app, main_layout[0]);

    // Split content: left chat area, right sidebar
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),                // Chat area (fills remaining)
            Constraint::Length(SIDEBAR_WIDTH), // Right sidebar
        ])
        .split(main_layout[1]);

    // Draw chat area (left)
    draw_chat(f, app, content_layout[0]);

    // Draw sidebar (right)
    draw_sidebar(f, app, content_layout[1]);

    // Draw input area
    draw_input(f, app, main_layout[2]);

    // Draw footer with keyboard shortcuts
    draw_footer(f, main_layout[3]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    // Crush-style ASCII art with diagonal lines pattern
    let banner_lines: Vec<Line> = banner::BANNER_CRUSH
        .lines()
        .enumerate()
        .map(|(i, line)| {
            // Gradient from magenta to purple
            let color = match i % 4 {
                0 => ACCENT_MAGENTA,
                1 => ACCENT_PURPLE,
                2 => ACCENT_PINK,
                _ => ACCENT_PURPLE,
            };
            Line::from(Span::styled(
                line,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    // Simple bottom border
    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(BORDER_COLOR));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Banner paragraph
    let banner_paragraph = Paragraph::new(banner_lines).alignment(Alignment::Left);

    let banner_area = Rect {
        x: inner.x + 2,
        y: inner.y,
        width: inner.width.saturating_sub(4),
        height: inner.height,
    };
    f.render_widget(banner_paragraph, banner_area);
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    // Chat area with just a left accent line (Crush style)
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(ACCENT_PURPLE));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    // Padding inside
    let chat_area = Rect {
        x: inner_area.x + 1,
        y: inner_area.y,
        width: inner_area.width.saturating_sub(2),
        height: inner_area.height,
    };

    // Calculate visible messages
    let max_lines = chat_area.height as usize;
    let mut items = Vec::new();
    let mut line_count = 0;

    for msg in app.messages.iter().rev().skip(app.scroll_offset) {
        if line_count >= max_lines {
            break;
        }

        let (prefix, accent_color, icon) = match msg.message_type {
            MessageType::User => ("you", ACCENT_MAGENTA, ""),
            MessageType::Assistant => ("assistant", ACCENT_PURPLE, ""),
            MessageType::System => ("system", TEXT_DIM, ""),
            MessageType::Error => ("error", STATUS_RED, ""),
            MessageType::Tool => ("tool", STATUS_AMBER, ""),
            MessageType::Orchestration => ("orchestrator", ACCENT_PINK, ""),
        };

        let time = msg.timestamp.format("%H:%M");

        // Wrap content
        let width = chat_area.width.saturating_sub(2) as usize;
        let wrapped = wrap(&msg.content, width);

        for (i, line) in wrapped.iter().enumerate() {
            if line_count >= max_lines {
                break;
            }

            if i == 0 {
                // First line with timestamp and prefix
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", time), Style::default().fg(TEXT_DIM)),
                    Span::styled(
                        format!("{}{}", icon, prefix),
                        Style::default()
                            .fg(accent_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" ", Style::default()),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_PRIMARY)),
                ])));
            } else {
                // Continuation lines
                items.push(ListItem::new(Line::from(vec![
                    Span::raw("      "), // Indent to align with content
                    Span::styled(line.to_string(), Style::default().fg(TEXT_PRIMARY)),
                ])));
            }
            line_count += 1;
        }

        // Add spacing between messages
        if line_count < max_lines {
            items.push(ListItem::new(Line::from("")));
            line_count += 1;
        }
    }

    items.reverse();

    let list = List::new(items);
    f.render_widget(list, chat_area);
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    // Sidebar with subtle background
    let block = Block::default()
        .borders(Borders::LEFT)
        .border_style(Style::default().fg(BORDER_COLOR))
        .style(Style::default().bg(SIDEBAR_BG));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split sidebar into sections
    let sidebar_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // Session info
            Constraint::Length(5), // Model info
            Constraint::Min(0),    // Modified files / Tasks
        ])
        .split(inner);

    // Draw session info section
    draw_session_info(f, app, sidebar_layout[0]);

    // Draw model info section
    draw_model_info(f, app, sidebar_layout[1]);

    // Draw tasks/tools section
    draw_tasks_section(f, app, sidebar_layout[2]);
}

fn draw_session_info(f: &mut Frame, app: &App, area: Rect) {
    let session_name = if app.session_status.active {
        "Active Session"
    } else {
        "New Session"
    };

    let status_dot = if app.session_status.active { "" } else { "" };
    let status_color = if app.session_status.active {
        STATUS_GREEN
    } else {
        TEXT_DIM
    };

    let content = vec![
        Line::from(vec![
            Span::styled(
                format!("{} ", status_dot),
                Style::default().fg(status_color),
            ),
            Span::styled(
                session_name,
                Style::default()
                    .fg(TEXT_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled("  Path ", Style::default().fg(TEXT_DIM))]),
        Line::from(vec![Span::styled(
            format!("  {}", truncate_path(&app.project_path, 26)),
            Style::default().fg(TEXT_SECONDARY),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Uptime ", Style::default().fg(TEXT_DIM)),
            Span::styled(
                &app.session_status.uptime,
                Style::default().fg(TEXT_SECONDARY),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(content);
    f.render_widget(
        paragraph,
        Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        },
    );
}

fn draw_model_info(f: &mut Frame, app: &App, area: Rect) {
    let thinking_status = if app.is_thinking { "On" } else { "Off" };
    let thinking_color = if app.is_thinking {
        STATUS_GREEN
    } else {
        TEXT_DIM
    };

    let content = vec![
        Line::from(vec![Span::styled(
            "  Model ",
            Style::default().fg(TEXT_DIM),
        )]),
        Line::from(vec![Span::styled(
            "  Claude Sonnet 4",
            Style::default().fg(ACCENT_PURPLE),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Thinking ", Style::default().fg(TEXT_DIM)),
            Span::styled(thinking_status, Style::default().fg(thinking_color)),
        ]),
    ];

    let paragraph = Paragraph::new(content);
    f.render_widget(
        paragraph,
        Rect {
            x: area.x + 1,
            y: area.y,
            width: area.width.saturating_sub(2),
            height: area.height,
        },
    );
}

fn draw_tasks_section(f: &mut Frame, app: &App, area: Rect) {
    // Section header
    let header = if !app.background_tasks.is_empty() {
        let active = app.get_active_tasks_count();
        let completed = app.get_completed_tasks_count();
        format!("  Tasks ({}/{})", completed, app.background_tasks.len())
    } else if !app.tool_executions.is_empty() {
        format!("  Tools ({})", app.tool_executions.len())
    } else {
        "  Activity".to_string()
    };

    let mut content = vec![
        Line::from(Span::styled(header, Style::default().fg(TEXT_DIM))),
        Line::from(""),
    ];

    // Show tasks or tools
    let max_items = (area.height.saturating_sub(3)) as usize;

    if !app.background_tasks.is_empty() {
        for task in app.background_tasks.iter().rev().take(max_items) {
            let (icon, color) = match &task.status {
                super::messages::BackgroundTaskStatus::Pending => ("", TEXT_DIM),
                super::messages::BackgroundTaskStatus::Running => ("", STATUS_AMBER),
                super::messages::BackgroundTaskStatus::Completed => ("", STATUS_GREEN),
                super::messages::BackgroundTaskStatus::Failed(_) => ("", STATUS_RED),
            };

            let desc = truncate_str(&task.description, 24);
            content.push(Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                Span::styled(desc, Style::default().fg(TEXT_SECONDARY)),
            ]));
        }
    } else if !app.tool_executions.is_empty() {
        for tool in app.tool_executions.iter().rev().take(max_items) {
            let (icon, color) = match tool.status {
                super::messages::ToolStatus::Running => ("", STATUS_AMBER),
                super::messages::ToolStatus::Success => ("", STATUS_GREEN),
                super::messages::ToolStatus::Failed => ("", STATUS_RED),
            };

            let name = truncate_str(&tool.tool_name, 24);
            content.push(Line::from(vec![
                Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                Span::styled(name, Style::default().fg(TEXT_SECONDARY)),
            ]));
        }
    } else {
        content.push(Line::from(vec![Span::styled(
            "  No activity",
            Style::default().fg(TEXT_DIM),
        )]));
    }

    let paragraph = Paragraph::new(content);
    f.render_widget(
        paragraph,
        Rect {
            x: area.x,
            y: area.y,
            width: area.width.saturating_sub(1),
            height: area.height,
        },
    );
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    // Input area with left accent line
    let block = Block::default()
        .borders(Borders::LEFT | Borders::TOP)
        .border_style(Style::default().fg(BORDER_COLOR));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Draw input content
    let input_content = if app.is_thinking {
        let frames = banner::get_processing_frames();
        let frame = frames[app.animation_frame % frames.len()];
        let dots = ".".repeat((app.animation_frame / 15) % 4);

        let processing_text = if app.processing_message.is_empty() {
            format!("thinking{}", dots)
        } else {
            format!("{}{}", app.processing_message.to_lowercase(), dots)
        };

        vec![
            Span::styled(" ", Style::default()),
            Span::styled(frame, Style::default().fg(STATUS_AMBER)),
            Span::styled(" ", Style::default()),
            Span::styled(processing_text, Style::default().fg(STATUS_AMBER)),
        ]
    } else {
        let cursor = if app.animation_frame % 20 < 10 {
            ""
        } else {
            " "
        };
        vec![
            Span::styled(
                " > ",
                Style::default()
                    .fg(ACCENT_PURPLE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.input, Style::default().fg(TEXT_PRIMARY)),
            Span::styled(cursor, Style::default().fg(ACCENT_PURPLE)),
        ]
    };

    let paragraph = Paragraph::new(Line::from(input_content));
    f.render_widget(paragraph, inner);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    // Crush-style keyboard shortcuts bar
    let shortcuts = vec![
        ("esc", "cancel"),
        ("tab", "focus"),
        ("ctrl+c", "quit"),
        ("/orch", "orchestrate"),
        ("", "scroll"),
    ];

    let mut spans = Vec::new();
    spans.push(Span::styled(" ", Style::default()));

    for (i, (key, action)) in shortcuts.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", Style::default().fg(TEXT_DIM)));
        }
        spans.push(Span::styled(*key, Style::default().fg(TEXT_SECONDARY)));
        spans.push(Span::styled(" ", Style::default()));
        spans.push(Span::styled(*action, Style::default().fg(TEXT_DIM)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).style(Style::default().bg(SIDEBAR_BG));

    f.render_widget(paragraph, area);
}

// Helper functions
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() > 2 {
            format!(".../{}", parts.last().unwrap_or(&""))
        } else {
            format!("...{}", &path[path.len().saturating_sub(max_len - 3)..])
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
