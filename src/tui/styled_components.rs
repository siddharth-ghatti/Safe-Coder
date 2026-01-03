use super::theme::{Theme, TailwindColors};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        block::{Position, Title},
        Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap,
    },
    Frame,
};

/// Enhanced styling utilities for the TUI
pub struct StyledComponents;

impl StyledComponents {
    /// Create a styled block with TailwindCSS-inspired design
    pub fn card<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
        Block::default()
            .title(
                Title::from(Span::styled(
                    title,
                    Style::default()
                        .fg(theme.colors.on_surface)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Left)
                .position(Position::Top),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.colors.border))
            .style(Style::default().bg(theme.colors.surface))
    }

    /// Create a focused/active card
    pub fn card_focused<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
        Block::default()
            .title(
                Title::from(Span::styled(
                    title,
                    Style::default()
                        .fg(theme.colors.primary)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Left)
                .position(Position::Top),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .border_style(Style::default().fg(theme.colors.border_focus))
            .style(Style::default().bg(theme.colors.surface))
    }

    /// Create an error card
    pub fn card_error<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
        Block::default()
            .title(
                Title::from(Span::styled(
                    title,
                    Style::default()
                        .fg(theme.colors.error)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Left)
                .position(Position::Top),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.colors.border_error))
            .style(Style::default().bg(theme.colors.surface))
    }

    /// Create a badge/chip component
    pub fn badge(text: &str, color: Color, bg_color: Color) -> Span {
        Span::styled(
            format!(" {} ", text),
            Style::default()
                .fg(color)
                .bg(bg_color)
                .add_modifier(Modifier::BOLD),
        )
    }

    /// Create status badges
    pub fn status_badge<'a>(status: &'a str, theme: &Theme) -> Span<'a> {
        match status.to_lowercase().as_str() {
            "success" | "completed" | "done" | "ok" => {
                Self::badge(status, theme.colors.on_primary, theme.colors.success)
            }
            "error" | "failed" | "fail" => {
                Self::badge(status, theme.colors.on_primary, theme.colors.error)
            }
            "warning" | "warn" | "pending" => {
                Self::badge(status, theme.colors.on_primary, theme.colors.warning)
            }
            "info" | "running" | "active" => {
                Self::badge(status, theme.colors.on_primary, theme.colors.info)
            }
            _ => Self::badge(status, theme.colors.on_surface, theme.colors.secondary),
        }
    }

    /// Create a progress bar with TailwindCSS styling
    pub fn progress_bar(percent: f64, theme: &Theme) -> Gauge {
        let color = if percent < 0.3 {
            theme.colors.error
        } else if percent < 0.7 {
            theme.colors.warning
        } else {
            theme.colors.success
        };

        Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(theme.colors.border)),
            )
            .gauge_style(Style::default().fg(color).bg(theme.colors.surface_variant))
            .percent((percent * 100.0) as u16)
            .label(format!("{:.1}%", percent * 100.0))
            .style(Style::default().bg(theme.colors.surface))
    }

    /// Create a styled list item
    pub fn list_item_with_status<'a>(text: &'a str, status: Option<&'a str>, theme: &Theme) -> ListItem<'a> {
        let mut spans = vec![Span::styled(
            text,
            Style::default().fg(theme.colors.on_surface),
        )];

        if let Some(status) = status {
            spans.push(Span::raw(" "));
            spans.push(Self::status_badge(status, theme));
        }

        ListItem::new(Line::from(spans))
    }

    /// Create a syntax highlighted code block (basic)
    pub fn code_block<'a>(code: &'a str, theme: &Theme) -> Paragraph<'a> {
        Paragraph::new(code)
            .style(
                Style::default()
                    .fg(TailwindColors::green(400))
                    .bg(theme.colors.surface_variant),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(theme.colors.border))
                    .title("Code")
                    .title_style(Style::default().fg(theme.colors.secondary)),
            )
            .wrap(Wrap { trim: true })
    }

    /// Create a modal/popup overlay
    pub fn modal<'a>(title: &'a str, theme: &Theme) -> Block<'a> {
        Block::default()
            .title(
                Title::from(Span::styled(
                    title,
                    Style::default()
                        .fg(theme.colors.on_surface)
                        .add_modifier(Modifier::BOLD),
                ))
                .alignment(Alignment::Center),
            )
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(theme.colors.primary))
            .style(
                Style::default()
                    .bg(theme.colors.surface)
                    .add_modifier(Modifier::BOLD),
            )
    }

    /// Create an input field
    pub fn input_field<'a>(placeholder: &'a str, focused: bool, theme: &Theme) -> Block<'a> {
        let (border_color, border_type) = if focused {
            (theme.colors.border_focus, BorderType::Thick)
        } else {
            (theme.colors.border, BorderType::Plain)
        };

        Block::default()
            .title(
                Title::from(Span::styled(
                    placeholder,
                    Style::default().fg(theme.colors.secondary),
                ))
                .position(Position::Top),
            )
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(theme.colors.surface))
    }

    /// Create a notification/toast
    pub fn notification<'a>(message: &'a str, notification_type: &str, theme: &Theme) -> Paragraph<'a> {
        let (bg_color, fg_color, border_color) = match notification_type {
            "success" => (
                theme.colors.success,
                theme.colors.on_primary,
                theme.colors.success,
            ),
            "error" => (theme.colors.error, theme.colors.on_primary, theme.colors.error),
            "warning" => (
                theme.colors.warning,
                theme.colors.on_primary,
                theme.colors.warning,
            ),
            _ => (
                theme.colors.info,
                theme.colors.on_primary,
                theme.colors.info,
            ),
        };

        Paragraph::new(message)
            .style(Style::default().fg(fg_color).bg(bg_color))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(border_color)),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
    }

    /// Helper to center a rect
    pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    /// Create a gradient effect using different shades
    pub fn gradient_text(text: &str, start_shade: u16, end_shade: u16) -> Vec<Span> {
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len();
        let mut spans = Vec::new();

        for (i, &ch) in chars.iter().enumerate() {
            let ratio = if len > 1 { i as f32 / (len - 1) as f32 } else { 0.0 };
            let shade = start_shade as f32 + (end_shade as f32 - start_shade as f32) * ratio;
            let color = TailwindColors::blue(shade as u16);
            
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        spans
    }
}

/// Layout utilities inspired by TailwindCSS
pub struct LayoutUtils;

impl LayoutUtils {
    /// Create a responsive grid layout
    pub fn grid_layout(area: Rect, cols: u16, rows: u16) -> Vec<Vec<Rect>> {
        let col_constraints: Vec<Constraint> = 
            (0..cols).map(|_| Constraint::Percentage(100 / cols)).collect();
        let row_constraints: Vec<Constraint> = 
            (0..rows).map(|_| Constraint::Percentage(100 / rows)).collect();

        let rows_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(row_constraints)
            .split(area);

        rows_layout
            .into_iter()
            .map(|row| {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(col_constraints.clone())
                    .split(*row)
                    .to_vec()
            })
            .collect()
    }

    /// Create a sidebar layout
    pub fn sidebar_layout(area: Rect, sidebar_width: u16) -> [Rect; 2] {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(sidebar_width),
                Constraint::Min(0),
            ])
            .split(area);
        [chunks[0], chunks[1]]
    }

    /// Create a header/content/footer layout
    pub fn page_layout(area: Rect, header_height: u16, footer_height: u16) -> [Rect; 3] {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Min(0),
                Constraint::Length(footer_height),
            ])
            .split(area);
        [chunks[0], chunks[1], chunks[2]]
    }

    /// Create a card with padding
    pub fn card_with_padding(area: Rect, padding: u16) -> Rect {
        area.inner(Margin {
            vertical: padding,
            horizontal: padding,
        })
    }
}