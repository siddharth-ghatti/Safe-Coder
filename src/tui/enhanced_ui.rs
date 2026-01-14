use super::{
    markdown::{has_markdown, render_markdown_lines},
    styled_components::{LayoutUtils, StyledComponents},
    theme::Theme,
    App,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Enhanced UI with TailwindCSS-inspired styling
pub fn draw_enhanced(f: &mut Frame, app: &mut App, theme: &Theme) {
    let size = f.area();

    // Main layout: header, content, footer (reduced heights for tighter UI)
    let main_layout = LayoutUtils::page_layout(size, 2, 2);
    let [header_area, content_area, footer_area] = main_layout;

    // Draw header
    draw_header(f, header_area, app, theme);

    // Content layout: sidebar and main content (reduced sidebar width)
    let content_layout = LayoutUtils::sidebar_layout(content_area, 26);
    let [sidebar_area, main_area] = content_layout;

    // Draw sidebar
    draw_enhanced_sidebar(f, sidebar_area, app, theme);

    // Draw main content
    draw_enhanced_main_content(f, main_area, app, theme);

    // Draw footer
    draw_footer(f, footer_area, app, theme);

    // Draw any modals/overlays
    if app.show_help {
        draw_help_modal(f, theme);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    // Title only - mode moved to sidebar
    let title_spans = StyledComponents::gradient_text("Safe Coder", 400, 600);
    let title = Paragraph::new(Line::from(title_spans))
        .style(Style::default().bg(theme.colors.surface))
        .alignment(Alignment::Center)
        .block(StyledComponents::card("", theme).borders(Borders::BOTTOM));

    f.render_widget(title, area);
}

fn draw_enhanced_sidebar(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let sidebar_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Mode indicator
            Constraint::Length(8), // Connection status
            Constraint::Length(6), // Token usage
            Constraint::Min(0),    // Session info
        ])
        .split(area);

    // Mode indicator card
    let mode = app.agent_mode;
    let mode_color = match mode {
        crate::tools::AgentMode::Plan => theme.colors.info,
        crate::tools::AgentMode::Build => theme.colors.success,
    };

    let mode_description = match mode {
        crate::tools::AgentMode::Plan => "Read-only exploration",
        crate::tools::AgentMode::Build => "Full tool access",
    };

    let mode_text = vec![
        Line::from(vec![Span::styled(
            format!("{}", mode.short_name()),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![Span::styled(
            mode_description,
            Style::default().fg(theme.colors.secondary),
        )]),
    ];

    let mode_card = Paragraph::new(mode_text)
        .block(StyledComponents::card("Mode", theme))
        .alignment(Alignment::Center);

    f.render_widget(mode_card, sidebar_layout[0]);

    // Connection Status Card
    let connection_block = if app.sidebar_state.connections.has_connected_lsp() {
        StyledComponents::card("Connection", theme)
    } else {
        StyledComponents::card_error("Connection", theme)
    };

    let status_text = if app.sidebar_state.connections.has_connected_lsp() {
        let lsp_count = app.sidebar_state.connections.connected_lsp_count();
        vec![
            Line::from(vec![
                Span::raw("Status: "),
                StyledComponents::status_badge("Connected", theme),
            ]),
            Line::from(vec![
                Span::raw("LSP Servers: "),
                Span::styled(
                    format!("{}", lsp_count),
                    Style::default()
                        .fg(theme.colors.primary)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::raw("Status: "),
                StyledComponents::status_badge("Disconnected", theme),
            ]),
            Line::from(Span::styled(
                "No LSP servers connected",
                Style::default().fg(theme.colors.error),
            )),
        ]
    };

    let connection_status = Paragraph::new(status_text)
        .block(connection_block)
        .wrap(Wrap { trim: true });

    f.render_widget(connection_status, sidebar_layout[1]);

    // Token Usage Card with Progress Bar
    let token_usage = &app.sidebar_state.token_usage;
    let usage_ratio = if token_usage.context_window > 0 {
        token_usage.total_tokens as f64 / token_usage.context_window as f64
    } else {
        0.0
    };

    let token_block = StyledComponents::card("Token Usage", theme);
    let token_text = vec![Line::from(vec![
        Span::raw(format!("{}/", token_usage.total_tokens)),
        Span::styled(
            format!("{}", token_usage.context_window),
            Style::default().fg(theme.colors.secondary),
        ),
    ])];

    let token_info = Paragraph::new(token_text).block(token_block);
    f.render_widget(token_info, sidebar_layout[2]);

    // Progress bar below token info
    let progress_area = Rect {
        x: sidebar_layout[2].x + 1,
        y: sidebar_layout[2].y + sidebar_layout[2].height - 2,
        width: sidebar_layout[2].width - 2,
        height: 1,
    };
    let progress = StyledComponents::progress_bar(usage_ratio, theme);
    f.render_widget(progress, progress_area);

    // Session Info
    let session_id = app.current_session_id.as_deref().unwrap_or("None");
    let messages_count = app.messages.len();
    let tasks_count = app.background_tasks.len();

    let session_text = format!("Session: {}", session_id);
    let messages_text = format!("Messages: {}", messages_count);
    let tasks_text = format!("{}", tasks_count);

    let session_items: Vec<ListItem> = vec![
        StyledComponents::list_item_with_status(&session_text, None, theme),
        StyledComponents::list_item_with_status(&messages_text, None, theme),
        StyledComponents::list_item_with_status("Background Tasks", Some(&tasks_text), theme),
    ];

    let session_list = List::new(session_items)
        .block(StyledComponents::card("Session", theme))
        .style(Style::default().fg(theme.colors.on_surface));

    f.render_widget(session_list, sidebar_layout[3]);
}

fn draw_enhanced_main_content(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Messages
            Constraint::Length(3), // Input
        ])
        .split(area);

    // Messages area
    let messages_block = if app.is_focused_on_input() {
        StyledComponents::card("Chat", theme)
    } else {
        StyledComponents::card_focused("Chat", theme)
    };

    let messages_area = LayoutUtils::card_with_padding(main_layout[0], 1);

    // Enhanced message rendering with markdown support
    let mut message_lines = Vec::new();

    for (i, message) in app.visible_messages().enumerate() {
        let style = match message.message_type {
            super::MessageType::User => Style::default()
                .fg(theme.colors.primary)
                .add_modifier(Modifier::BOLD),
            super::MessageType::Assistant => Style::default().fg(theme.colors.on_surface),
            super::MessageType::System => Style::default()
                .fg(theme.colors.secondary)
                .add_modifier(Modifier::ITALIC),
            super::MessageType::Error => theme.styles.error,
            super::MessageType::Tool => Style::default().fg(theme.colors.accent),
            super::MessageType::Orchestration => Style::default().fg(theme.colors.info),
        };

        let prefix = match message.message_type {
            super::MessageType::User => "❯ ",
            super::MessageType::Assistant => "🤖 ",
            super::MessageType::System => "ℹ️ ",
            super::MessageType::Error => "❌ ",
            super::MessageType::Tool => "🔧 ",
            super::MessageType::Orchestration => "🎯 ",
        };

        // Check if this is an assistant message with markdown content
        if matches!(message.message_type, super::MessageType::Assistant)
            && has_markdown(&message.content)
        {
            // Render markdown content for assistant messages
            message_lines.push(Line::from(Span::styled(prefix, style)));

            let md_lines = render_markdown_lines(&message.content);
            for md_line in md_lines {
                // Add a small indent for markdown content
                let mut spans = vec![Span::styled("  ", Style::default())];
                spans.extend(md_line.spans);
                message_lines.push(Line::from(spans));
            }
        } else {
            // Regular text rendering for non-markdown content
            message_lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(&message.content, style),
            ]));
        }

        // Add spacing between messages
        if i < app.visible_messages().count() - 1 {
            message_lines.push(Line::from(""));
        }
    }

    // Add thinking indicator
    if app.is_thinking {
        let thinking_word = app.spinner.current().to_string();
        let thinking_text = format!("🤔 {}", thinking_word);
        let thinking_spans = StyledComponents::gradient_text(&thinking_text, 400, 600)
            .into_iter()
            .map(|s| Span::styled(s.content.to_string(), s.style))
            .collect::<Vec<_>>();
        message_lines.push(Line::from(""));
        message_lines.push(Line::from(thinking_spans));
    }

    let messages_paragraph = Paragraph::new(message_lines)
        .block(messages_block)
        .wrap(Wrap { trim: true })
        .scroll((app.scroll_offset as u16, 0));

    f.render_widget(messages_paragraph, main_layout[0]);

    // Input area
    let input_block = if app.is_focused_on_input() {
        StyledComponents::input_field("Type your message...", true, theme)
    } else {
        StyledComponents::input_field("Type your message...", false, theme)
    };

    let input_content = if app.input.is_empty() {
        Span::styled(
            "Type your message...",
            Style::default()
                .fg(theme.colors.secondary)
                .add_modifier(Modifier::ITALIC),
        )
    } else {
        Span::styled(&app.input, Style::default().fg(theme.colors.on_surface))
    };

    let input_paragraph = Paragraph::new(Line::from(input_content)).block(input_block);

    f.render_widget(input_paragraph, main_layout[1]);

    // Cursor position
    if app.is_focused_on_input() {
        f.set_cursor_position(Position {
            x: main_layout[1].x + app.input.len() as u16 + 1,
            y: main_layout[1].y + 1,
        });
    }
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let footer_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),     // Status
            Constraint::Length(35), // Context indicator
        ])
        .split(area);

    // Status
    let status = Paragraph::new(app.status.clone())
        .style(Style::default().fg(theme.colors.on_surface))
        .alignment(Alignment::Left);
    f.render_widget(status, footer_layout[0]);

    // Context left until auto-compact indicator (like Claude Code)
    let context_left = app.sidebar_state.token_usage.context_left_until_compact();
    let context_text = format!("Context left until auto-compact: {}%", context_left);
    let context_indicator = Paragraph::new(context_text)
        .style(Style::default().fg(theme.colors.secondary))
        .alignment(Alignment::Right);
    f.render_widget(context_indicator, footer_layout[1]);
}

fn draw_help_modal(f: &mut Frame, theme: &Theme) {
    let area = StyledComponents::centered_rect(60, 50, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    let help_content = vec![
        Line::from("Keyboard Shortcuts:"),
        Line::from(""),
        Line::from("• Tab - Switch focus between panels"),
        Line::from("• ↑/↓ - Scroll messages"),
        Line::from("• Page Up/Down - Scroll by page"),
        Line::from("• Enter - Send message"),
        Line::from("• Esc - Close this help"),
        Line::from("• F1 - Cycle theme (Dark → Light → Monokai)"),
        Line::from("• Ctrl+G - Cycle agent mode (Plan ↔ Build)"),
        Line::from("• Ctrl+C - Quit application"),
        Line::from(""),
        Line::from("Agent Modes:"),
        Line::from("• PLAN - Read-only exploration and planning"),
        Line::from("• BUILD - Full execution with file modifications"),
        Line::from(""),
        Line::from("Commands:"),
        Line::from("• /orchestrate <task> - Run orchestrated task"),
        Line::from("• /orch <task> - Shortcut for orchestrate"),
        Line::from("• exit - Quit application"),
    ];

    let help_paragraph = Paragraph::new(help_content)
        .block(StyledComponents::modal("Help", theme))
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.colors.on_surface));

    f.render_widget(help_paragraph, area);
}
