use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};
use textwrap::wrap;

use super::app::App;
use super::enhanced_ui::draw_enhanced;
use super::messages::MessageType;
use super::theme::Theme;

// Claude Code inspired color scheme - clean and minimal
const ACCENT_BLUE: Color = Color::Rgb(100, 149, 237); // Cornflower blue for user
const ACCENT_PURPLE: Color = Color::Rgb(180, 120, 200); // Purple for assistant
const ACCENT_GREEN: Color = Color::Rgb(120, 200, 140); // Green for success/system
const ACCENT_AMBER: Color = Color::Rgb(220, 180, 100); // Amber for tools/warnings
const ACCENT_RED: Color = Color::Rgb(220, 100, 100); // Red for errors

const TEXT_PRIMARY: Color = Color::Rgb(220, 220, 220); // Main text
const TEXT_DIM: Color = Color::Rgb(100, 100, 100); // Dimmed text
const BORDER_DIM: Color = Color::Rgb(60, 60, 65); // Subtle borders

pub fn draw(f: &mut Frame, app: &mut App) {
    // Use enhanced UI if available, fall back to simple UI
    let theme = app.theme_manager.get_current_theme();
    draw_enhanced(f, app, &theme);
}

// Keep the original simple UI as a fallback
pub fn draw_simple(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Calculate dynamic input height based on wrapped content
    let input_height = calculate_input_height(app, size.width);

    // Simple layout: chat area takes most space, input at bottom
    // No header, no sidebar - just like Claude Code
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),                          // Chat area (fills available space)
            Constraint::Length(input_height.max(3)),     // Input area (dynamic, min 3)
        ])
        .split(size);

    draw_chat(f, app, main_layout[0]);
    draw_input(f, app, main_layout[1]);
}

/// Calculate the required height for the input area based on wrapped content
fn calculate_input_height(app: &App, terminal_width: u16) -> u16 {
    // Account for borders and padding
    let available_width = terminal_width.saturating_sub(4); // 4 for borders and padding
    let available_width = available_width.max(20); // Ensure minimum usable width
    
    if available_width == 0 {
        return 3; // Minimum height
    }
    
    let prompt_prefix = "> ";
    let input_with_cursor = format!("{} ", app.input); // Add space for cursor
    
    // Calculate wrapped lines using textwrap
    let content_width = (available_width as usize).saturating_sub(prompt_prefix.len());
    let content_width = content_width.max(10); // Ensure reasonable minimum
    
    let wrapped_lines = if app.input.is_empty() {
        1 // Just the prompt line
    } else {
        let wrapped = wrap(&input_with_cursor, content_width);
        wrapped.len().max(1)
    };
    
    // Add 2 for border, ensure minimum height of 3, maximum of 8
    let height = (wrapped_lines as u16 + 2).max(3).min(8);
    height
}

fn draw_chat(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 2 || area.width < 10 {
        return;
    }

    // Calculate available width for content (more generous than input area)
    let available_width = area.width.saturating_sub(2); // Account for borders and padding
    let effective_width = (available_width as usize * 85) / 100; // Use 85% of width for better readability
    let effective_width = effective_width.max(30); // Ensure reasonable minimum width

    // Build all lines from messages
    let mut all_lines: Vec<Line> = Vec::new();

    for msg in app.messages.iter() {
        // Tool messages get special compact formatting
        if msg.message_type == MessageType::Tool {
            let tool_wrapped = wrap(&msg.content, effective_width.saturating_sub(4)); // Account for "  ⚙ " prefix
            for line in tool_wrapped.iter() {
                all_lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled("⚙ ", Style::default().fg(ACCENT_AMBER)),
                    Span::styled(line.to_string(), Style::default().fg(TEXT_DIM)),
                ]));
            }
            continue;
        }

        let (role_label, role_color) = match msg.message_type {
            MessageType::User => ("you", ACCENT_BLUE),
            MessageType::Assistant => ("assistant", ACCENT_PURPLE),
            MessageType::System => ("system", ACCENT_GREEN),
            MessageType::Error => ("error", ACCENT_RED),
            MessageType::Tool => ("tool", ACCENT_AMBER), // Won't reach here
            MessageType::Orchestration => ("orchestrator", ACCENT_PURPLE),
        };

        // Role header line (like Claude Code: "> you" or "assistant")
        let role_prefix = match msg.message_type {
            MessageType::User => "> ",
            MessageType::Error => "! ",
            _ => "",
        };

        all_lines.push(Line::from(vec![Span::styled(
            format!("{}{}", role_prefix, role_label),
            Style::default().fg(role_color).add_modifier(Modifier::BOLD),
        )]));

        // Message content - wrapped to fit width
        let wrapped = wrap(&msg.content, effective_width.saturating_sub(2));
        for line in wrapped.iter() {
            all_lines.push(Line::from(vec![
                Span::styled("  ", Style::default()), // Indent content
                Span::styled(line.to_string(), Style::default().fg(TEXT_PRIMARY)),
            ]));
        }

        // Blank line between messages
        all_lines.push(Line::from(""));
    }

    // Show thinking indicator if active
    if app.is_thinking {
        let dots = ".".repeat((app.animation_frame / 10) % 4);
        let thinking_text = if app.processing_message.is_empty() {
            format!("thinking{}", dots)
        } else {
            format!("{}{}", app.processing_message.to_lowercase(), dots)
        };

        all_lines.push(Line::from(vec![Span::styled(
            "assistant",
            Style::default()
                .fg(ACCENT_PURPLE)
                .add_modifier(Modifier::BOLD),
        )]));
        all_lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(thinking_text, Style::default().fg(TEXT_DIM)),
        ]));
    }

    // Calculate visible portion
    let max_lines = area.height as usize;
    let total_lines = all_lines.len();

    // scroll_offset = 0 shows bottom (most recent), higher values scroll up
    let visible_start = if total_lines > max_lines {
        total_lines
            .saturating_sub(max_lines)
            .saturating_sub(app.scroll_offset)
    } else {
        0
    };
    let visible_end = (visible_start + max_lines).min(total_lines);

    let visible_lines: Vec<ListItem> = all_lines
        .get(visible_start..visible_end)
        .unwrap_or(&[])
        .iter()
        .map(|line| ListItem::new(line.clone()))
        .collect();

    // Render chat with minimal styling
    let list = List::new(visible_lines);
    f.render_widget(list, area);

    // Show scrollbar if content overflows
    if total_lines > max_lines {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█");

        let mut scrollbar_state = ScrollbarState::new(total_lines.saturating_sub(max_lines))
            .position(
                total_lines
                    .saturating_sub(max_lines)
                    .saturating_sub(app.scroll_offset),
            );

        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };

        f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    // Top border to separate from chat
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER_DIM));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Add some left padding
    let input_area = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    };

    // Create input text with prompt and cursor
    let cursor = if app.animation_frame % 20 < 10 { "█" } else { " " };

    // Split the input into lines for proper wrapping
    let available_width = input_area.width as usize;
    if available_width == 0 {
        return;
    }

    // Create lines with proper wrapping
    let mut lines = Vec::new();
    let prompt_prefix = "> ";
    
    if app.input.is_empty() {
        // Just show prompt and cursor
        lines.push(Line::from(vec![
            Span::styled(
                prompt_prefix,
                Style::default()
                    .fg(ACCENT_BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(cursor, Style::default().fg(ACCENT_BLUE)),
        ]));
    } else {
        // Wrap the input text properly
        let input_with_cursor = format!("{}{}", app.input, cursor);
        let wrapped_lines = wrap(&input_with_cursor, available_width.saturating_sub(prompt_prefix.len()));
        
        for (i, line) in wrapped_lines.iter().enumerate() {
            if i == 0 {
                // First line gets the prompt prefix
                lines.push(Line::from(vec![
                    Span::styled(
                        prompt_prefix,
                        Style::default()
                            .fg(ACCENT_BLUE)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        line.to_string(),
                        Style::default().fg(TEXT_PRIMARY),
                    ),
                ]));
            } else {
                // Continuation lines get proper indentation
                let indent = " ".repeat(prompt_prefix.len());
                lines.push(Line::from(vec![
                    Span::styled(indent, Style::default()),
                    Span::styled(
                        line.to_string(),
                        Style::default().fg(TEXT_PRIMARY),
                    ),
                ]));
            }
        }
    }

    // Use Paragraph with the wrapped lines
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, input_area);
}
