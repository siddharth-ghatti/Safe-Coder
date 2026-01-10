//! Markdown rendering for the TUI
//!
//! This module provides markdown-to-ratatui conversion for rendering
//! formatted text in the terminal UI. Uses pulldown-cmark for parsing
//! and syntect for code syntax highlighting.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use std::sync::LazyLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Cached syntax set - expensive to load, so we do it once at startup
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(|| SyntaxSet::load_defaults_newlines());

/// Cached theme set - expensive to load, so we do it once at startup
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(|| ThemeSet::load_defaults());

/// Colors for markdown rendering (matching shell_ui palette)
const ACCENT_CYAN: Color = Color::Rgb(80, 200, 220);
const ACCENT_GREEN: Color = Color::Rgb(120, 200, 120);
const ACCENT_YELLOW: Color = Color::Rgb(220, 200, 100);
const ACCENT_MAGENTA: Color = Color::Rgb(180, 120, 200);
const ACCENT_BLUE: Color = Color::Rgb(100, 140, 200);
const TEXT_PRIMARY: Color = Color::Rgb(210, 210, 215);
const TEXT_SECONDARY: Color = Color::Rgb(150, 150, 160);
const TEXT_DIM: Color = Color::Rgb(100, 100, 110);
const BG_CODE: Color = Color::Rgb(38, 40, 50);
const BORDER_SUBTLE: Color = Color::Rgb(55, 58, 68);

/// Creates a sample markdown text for demonstration
pub fn create_sample_markdown() -> String {
    r#"# Markdown Rendering Demo

This is a demonstration of the **enhanced markdown rendering** capabilities in Safe Coder.

## Features

- **Bold text** and *italic text*
- `inline code` and code blocks
- Lists and task lists
- > Blockquotes for important information
- Tables and links

### Code Example

```rust
fn main() {
    println!("Hello, Safe Coder!");
    let greeting = "Markdown rendering works!";
    println!("{}", greeting);
}
```

### Task List

- [x] Implement markdown parsing
- [x] Add syntax highlighting
- [ ] Add more themes
- [x] Integrate with TUI

### Table Example

| Feature | Status | Notes |
|---------|--------|-------|
| Headers | ✅ | H1-H6 supported |
| Lists | ✅ | Ordered and unordered |
| Code | ✅ | With syntax highlighting |
| Tables | ✅ | Fully formatted |

> **Note:** This is rendered using pulldown-cmark and syntect for a rich terminal experience!

---

Try typing some markdown in the chat and see it rendered automatically!
"#
    .to_string()
}

/// Markdown renderer state
struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    current_line: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_depth: usize,
    list_counters: Vec<Option<u64>>, // None = unordered, Some(n) = ordered starting at n
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_content: String,
    in_blockquote: bool,
    blockquote_depth: usize,
    // Table state
    in_table: bool,
    table_row: Vec<String>,
    table_rows: Vec<Vec<String>>,
    in_table_header: bool,
    current_cell: String,
    // Note: syntax_set and theme_set are now global statics (SYNTAX_SET, THEME_SET)
}

impl MarkdownRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: Vec::new(),
            style_stack: vec![Style::default().fg(TEXT_PRIMARY)],
            list_depth: 0,
            list_counters: Vec::new(),
            in_code_block: false,
            code_block_lang: None,
            code_block_content: String::new(),
            in_blockquote: false,
            blockquote_depth: 0,
            in_table: false,
            table_row: Vec::new(),
            table_rows: Vec::new(),
            in_table_header: false,
            current_cell: String::new(),
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_line.is_empty() {
            // Add blockquote prefix if in blockquote
            if self.in_blockquote {
                let prefix = "│ ".repeat(self.blockquote_depth);
                let mut line = vec![Span::styled(prefix, Style::default().fg(ACCENT_CYAN))];
                line.extend(std::mem::take(&mut self.current_line));
                self.lines.push(Line::from(line));
            } else {
                self.lines
                    .push(Line::from(std::mem::take(&mut self.current_line)));
            }
        } else if self.in_blockquote {
            // Empty line in blockquote still gets prefix
            let prefix = "│ ".repeat(self.blockquote_depth);
            self.lines.push(Line::from(vec![Span::styled(
                prefix,
                Style::default().fg(ACCENT_CYAN),
            )]));
        } else {
            self.lines.push(Line::from(""));
        }
    }

    fn add_text(&mut self, text: &str) {
        let style = self.current_style();
        self.current_line
            .push(Span::styled(text.to_string(), style));
    }

    fn render_code_block(&mut self) {
        let content = std::mem::take(&mut self.code_block_content);
        let lang = self.code_block_lang.take();

        // Code block header
        let lang_display = lang.as_deref().unwrap_or("text");
        self.lines.push(Line::from(vec![
            Span::styled("┌─ ", Style::default().fg(BORDER_SUBTLE)),
            Span::styled(
                lang_display.to_string(),
                Style::default()
                    .fg(ACCENT_CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " ".to_string() + &"─".repeat(40),
                Style::default().fg(BORDER_SUBTLE),
            ),
        ]));

        // Syntax highlighting (using cached global statics)
        let syntax = lang
            .as_ref()
            .and_then(|l| SYNTAX_SET.find_syntax_by_token(l))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let theme = &THEME_SET.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        for line in LinesWithEndings::from(&content) {
            let mut spans = vec![Span::styled("│ ", Style::default().fg(BORDER_SUBTLE))];

            match highlighter.highlight_line(line, &SYNTAX_SET) {
                Ok(ranges) => {
                    for (style, text) in ranges {
                        let fg =
                            Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                        let mut ratatui_style = Style::default().fg(fg).bg(BG_CODE);

                        if style.font_style.contains(FontStyle::BOLD) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                        }
                        if style.font_style.contains(FontStyle::ITALIC) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                        }
                        if style.font_style.contains(FontStyle::UNDERLINE) {
                            ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                        }

                        spans.push(Span::styled(
                            text.trim_end_matches('\n').to_string(),
                            ratatui_style,
                        ));
                    }
                }
                Err(_) => {
                    spans.push(Span::styled(
                        line.trim_end_matches('\n').to_string(),
                        Style::default().fg(TEXT_PRIMARY).bg(BG_CODE),
                    ));
                }
            }

            self.lines.push(Line::from(spans));
        }

        // Code block footer
        self.lines.push(Line::from(vec![Span::styled(
            "└".to_string() + &"─".repeat(45),
            Style::default().fg(BORDER_SUBTLE),
        )]));
    }

    fn render_table(&mut self) {
        let rows = std::mem::take(&mut self.table_rows);
        if rows.is_empty() {
            return;
        }

        // Calculate column widths
        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths: Vec<usize> = vec![0; num_cols];

        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }

        // Minimum column width
        for w in &mut col_widths {
            *w = (*w).max(3);
        }

        // Build table border
        let top_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┬");

        let mid_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┼");

        let bottom_border: String = col_widths
            .iter()
            .map(|w| "─".repeat(*w + 2))
            .collect::<Vec<_>>()
            .join("┴");

        // Top border
        self.lines.push(Line::from(Span::styled(
            format!("┌{}┐", top_border),
            Style::default().fg(BORDER_SUBTLE),
        )));

        // Render rows
        for (row_idx, row) in rows.iter().enumerate() {
            let mut spans = vec![Span::styled("│", Style::default().fg(BORDER_SUBTLE))];

            for (col_idx, cell) in row.iter().enumerate() {
                let width = col_widths.get(col_idx).copied().unwrap_or(3);
                let padded = format!(" {:width$} ", cell, width = width);

                let style = if row_idx == 0 {
                    // Header row
                    Style::default()
                        .fg(ACCENT_CYAN)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT_PRIMARY)
                };

                spans.push(Span::styled(padded, style));
                spans.push(Span::styled("│", Style::default().fg(BORDER_SUBTLE)));
            }

            // Fill empty columns if row is shorter
            for col_idx in row.len()..num_cols {
                let width = col_widths.get(col_idx).copied().unwrap_or(3);
                spans.push(Span::styled(
                    " ".repeat(width + 2),
                    Style::default().fg(TEXT_PRIMARY),
                ));
                spans.push(Span::styled("│", Style::default().fg(BORDER_SUBTLE)));
            }

            self.lines.push(Line::from(spans));

            // Add separator after header
            if row_idx == 0 && rows.len() > 1 {
                self.lines.push(Line::from(Span::styled(
                    format!("├{}┤", mid_border),
                    Style::default().fg(BORDER_SUBTLE),
                )));
            }
        }

        // Bottom border
        self.lines.push(Line::from(Span::styled(
            format!("└{}┘", bottom_border),
            Style::default().fg(BORDER_SUBTLE),
        )));
    }

    fn process_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => {
                if self.in_code_block {
                    self.code_block_content.push_str(&text);
                } else if self.in_table {
                    self.current_cell.push_str(&text);
                } else {
                    self.add_text(&text);
                }
            }
            Event::Code(code) => {
                // Inline code
                self.current_line.push(Span::styled(
                    format!("`{}`", code),
                    Style::default().fg(ACCENT_CYAN).bg(BG_CODE),
                ));
            }
            Event::SoftBreak => {
                self.add_text(" ");
            }
            Event::HardBreak => {
                self.flush_line();
            }
            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(50),
                    Style::default().fg(BORDER_SUBTLE),
                )));
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_line();
                let (prefix, color) = match level {
                    HeadingLevel::H1 => ("# ", ACCENT_CYAN),
                    HeadingLevel::H2 => ("## ", ACCENT_GREEN),
                    HeadingLevel::H3 => ("### ", ACCENT_YELLOW),
                    HeadingLevel::H4 => ("#### ", ACCENT_MAGENTA),
                    HeadingLevel::H5 => ("##### ", ACCENT_BLUE),
                    HeadingLevel::H6 => ("###### ", TEXT_SECONDARY),
                };
                self.current_line
                    .push(Span::styled(prefix.to_string(), Style::default().fg(color)));
                self.push_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
            }
            Tag::Paragraph => {
                if !self.current_line.is_empty() || !self.lines.is_empty() {
                    self.flush_line();
                }
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                self.in_blockquote = true;
                self.blockquote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
            }
            Tag::List(start) => {
                if self.list_depth > 0 {
                    self.flush_line();
                }
                self.list_depth += 1;
                self.list_counters.push(start);
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));

                let bullet = if let Some(Some(n)) = self.list_counters.last_mut() {
                    let bullet = format!("{}. ", n);
                    *n += 1;
                    Span::styled(
                        format!("{}{}", indent, bullet),
                        Style::default().fg(ACCENT_YELLOW),
                    )
                } else {
                    Span::styled(format!("{}• ", indent), Style::default().fg(ACCENT_GREEN))
                };

                self.current_line.push(bullet);
            }
            Tag::Emphasis => {
                self.push_style(self.current_style().add_modifier(Modifier::ITALIC));
            }
            Tag::Strong => {
                self.push_style(self.current_style().add_modifier(Modifier::BOLD));
            }
            Tag::Strikethrough => {
                self.push_style(
                    self.current_style()
                        .fg(TEXT_DIM)
                        .add_modifier(Modifier::CROSSED_OUT),
                );
            }
            Tag::Link { .. } => {
                self.push_style(
                    Style::default()
                        .fg(ACCENT_BLUE)
                        .add_modifier(Modifier::UNDERLINED),
                );
                // We'll store the URL to display after the link text
                self.current_line
                    .push(Span::styled("[", Style::default().fg(TEXT_DIM)));
            }
            Tag::Table(_) => {
                self.flush_line();
                self.in_table = true;
                self.table_rows.clear();
            }
            Tag::TableHead => {
                self.in_table_header = true;
                self.table_row.clear();
            }
            Tag::TableRow => {
                self.table_row.clear();
            }
            Tag::TableCell => {
                self.current_cell.clear();
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.pop_style();
                self.flush_line();
            }
            TagEnd::Paragraph => {
                self.flush_line();
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
                if self.blockquote_depth == 0 {
                    self.in_blockquote = false;
                }
                self.flush_line();
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.render_code_block();
            }
            TagEnd::List(_) => {
                self.list_depth = self.list_depth.saturating_sub(1);
                self.list_counters.pop();
                if self.list_depth == 0 {
                    self.flush_line();
                }
            }
            TagEnd::Item => {
                // Items end naturally at the next item or list end
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Link => {
                self.pop_style();
                self.current_line
                    .push(Span::styled("]", Style::default().fg(TEXT_DIM)));
            }
            TagEnd::Table => {
                self.in_table = false;
                self.render_table();
            }
            TagEnd::TableHead => {
                self.in_table_header = false;
                if !self.table_row.is_empty() {
                    self.table_rows.push(std::mem::take(&mut self.table_row));
                }
            }
            TagEnd::TableRow => {
                if !self.table_row.is_empty() {
                    self.table_rows.push(std::mem::take(&mut self.table_row));
                }
            }
            TagEnd::TableCell => {
                self.table_row.push(std::mem::take(&mut self.current_cell));
            }
            _ => {}
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        if !self.current_line.is_empty() {
            self.flush_line();
        }
        self.lines
    }
}

/// Render markdown text to ratatui Text widget
pub fn render_markdown(input: &str) -> Text<'static> {
    Text::from(render_markdown_lines(input))
}

/// Render markdown and return as Vec of Lines for custom integration
pub fn render_markdown_lines(input: &str) -> Vec<Line<'static>> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, options);
    let mut renderer = MarkdownRenderer::new();

    for event in parser {
        renderer.process_event(event);
    }

    renderer.finish()
}

/// Render markdown with a left border prefix (for AI responses)
pub fn render_markdown_with_border(input: &str, border_color: Color) -> Vec<Line<'static>> {
    let lines = render_markdown_lines(input);

    lines
        .into_iter()
        .map(move |line| {
            let border_span = Span::styled("│ ", Style::default().fg(border_color));
            let mut spans = vec![border_span];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

/// Simple inline markdown parser for single-line text
/// Handles: **bold**, *italic*, `code`, ~~strikethrough~~
pub fn parse_inline_markdown(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut chars = text.chars().peekable();
    let mut current = String::new();

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                // Code span
                if !current.is_empty() {
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default().fg(TEXT_PRIMARY),
                    ));
                    current.clear();
                }
                let mut code = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '`' {
                        chars.next();
                        break;
                    }
                    code.push(chars.next().unwrap());
                }
                if !code.is_empty() {
                    spans.push(Span::styled(
                        code,
                        Style::default().fg(ACCENT_CYAN).bg(BG_CODE),
                    ));
                }
            }
            '*' => {
                if chars.peek() == Some(&'*') {
                    // Bold
                    chars.next();
                    if !current.is_empty() {
                        spans.push(Span::styled(
                            current.clone(),
                            Style::default().fg(TEXT_PRIMARY),
                        ));
                        current.clear();
                    }
                    let mut bold_text = String::new();
                    while let Some(&next) = chars.peek() {
                        if next == '*' {
                            chars.next();
                            if chars.peek() == Some(&'*') {
                                chars.next();
                                break;
                            }
                            bold_text.push('*');
                        } else {
                            bold_text.push(chars.next().unwrap());
                        }
                    }
                    if !bold_text.is_empty() {
                        spans.push(Span::styled(
                            bold_text,
                            Style::default()
                                .fg(TEXT_PRIMARY)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                } else {
                    // Italic
                    if !current.is_empty() {
                        spans.push(Span::styled(
                            current.clone(),
                            Style::default().fg(TEXT_PRIMARY),
                        ));
                        current.clear();
                    }
                    let mut italic_text = String::new();
                    while let Some(&next) = chars.peek() {
                        if next == '*' {
                            chars.next();
                            break;
                        }
                        italic_text.push(chars.next().unwrap());
                    }
                    if !italic_text.is_empty() {
                        spans.push(Span::styled(
                            italic_text,
                            Style::default()
                                .fg(TEXT_PRIMARY)
                                .add_modifier(Modifier::ITALIC),
                        ));
                    }
                }
            }
            '~' => {
                if chars.peek() == Some(&'~') {
                    // Strikethrough
                    chars.next();
                    if !current.is_empty() {
                        spans.push(Span::styled(
                            current.clone(),
                            Style::default().fg(TEXT_PRIMARY),
                        ));
                        current.clear();
                    }
                    let mut strike_text = String::new();
                    while let Some(&next) = chars.peek() {
                        if next == '~' {
                            chars.next();
                            if chars.peek() == Some(&'~') {
                                chars.next();
                                break;
                            }
                            strike_text.push('~');
                        } else {
                            strike_text.push(chars.next().unwrap());
                        }
                    }
                    if !strike_text.is_empty() {
                        spans.push(Span::styled(
                            strike_text,
                            Style::default()
                                .fg(TEXT_DIM)
                                .add_modifier(Modifier::CROSSED_OUT),
                        ));
                    }
                } else {
                    current.push(c);
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        spans.push(Span::styled(current, Style::default().fg(TEXT_PRIMARY)));
    }

    if spans.is_empty() {
        spans.push(Span::styled(String::new(), Style::default()));
    }

    spans
}

/// Check if a string contains markdown formatting
pub fn has_markdown(text: &str) -> bool {
    // Check for various markdown patterns
    text.contains("```")              // Code blocks
        || text.contains("**")        // Bold text
        || text.contains("__")        // Alternative bold
        || (text.contains('*') && text.matches('*').count() >= 2)  // Italic text
        || text.contains('`')         // Inline code
        || text.contains("# ")        // Headers (at start of line)
        || text.contains("\n# ")      // Headers (after newline)
        || text.contains("## ")       // H2-H6 headers
        || text.contains("- ")        // Unordered lists
        || text.contains("+ ")        // Alternative unordered lists
        || text.contains("* ")        // Alternative unordered lists (when not italic)
        || text.contains("~~")        // Strikethrough
        || text.contains("> ")        // Blockquotes
        || text.contains("1. ")       // Ordered lists
        || text.contains("2. ")       // More ordered list indicators
        || text.contains("[") && text.contains("](")  // Links
        || text.contains("![") && text.contains("](") // Images
        || (text.contains('|') && text.contains("---")) // Tables
        || text.contains("---")       // Horizontal rules
        || text.contains("- [ ]")     // Task lists (unchecked)
        || text.contains("- [x]")     // Task lists (checked)
        || text.contains("- [X]") // Task lists (checked, uppercase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_bold() {
        let spans = parse_inline_markdown("Hello **world**!");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "Hello ");
        assert_eq!(spans[1].content, "world");
        assert_eq!(spans[2].content, "!");
    }

    #[test]
    fn test_inline_code() {
        let spans = parse_inline_markdown("Use `cargo build` to compile");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content, "cargo build");
    }

    #[test]
    fn test_inline_italic() {
        let spans = parse_inline_markdown("This is *emphasized* text");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].content, "emphasized");
    }

    #[test]
    fn test_no_markdown() {
        let spans = parse_inline_markdown("Plain text without formatting");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "Plain text without formatting");
    }

    #[test]
    fn test_render_heading() {
        let lines = render_markdown_lines("# Hello World");
        assert!(!lines.is_empty());
        // Check that "Hello World" appears somewhere in the rendered output
        let all_text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_text.contains("Hello World"), "Got: {}", all_text);
    }

    #[test]
    fn test_render_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown_lines(md);
        // Should have header, code line, and footer
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_render_list() {
        let md = "- Item 1\n- Item 2\n- Item 3";
        let lines = render_markdown_lines(md);
        // Should have 3 list items
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_render_table() {
        let md =
            "| Header1 | Header2 |\n|---------|--------|\n| Cell1 | Cell2 |\n| Cell3 | Cell4 |";
        let lines = render_markdown_lines(md);
        // Should have top border, header, separator, 2 data rows, bottom border
        assert!(lines.len() >= 5, "Got {} lines", lines.len());
        // Check that content appears
        let all_text: String = lines
            .iter()
            .flat_map(|line| line.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(all_text.contains("Header1"), "Missing Header1");
        assert!(all_text.contains("Cell1"), "Missing Cell1");
    }

    #[test]
    fn test_enhanced_markdown_detection() {
        // Test various markdown patterns
        assert!(has_markdown("# Header"));
        assert!(has_markdown("text\n# Header"));
        assert!(has_markdown("## Subheader"));
        assert!(has_markdown("**bold**"));
        assert!(has_markdown("__bold__"));
        assert!(has_markdown("*italic*"));
        assert!(has_markdown("`code`"));
        assert!(has_markdown("```rust\ncode\n```"));
        assert!(has_markdown("- list item"));
        assert!(has_markdown("+ list item"));
        assert!(has_markdown("1. numbered"));
        assert!(has_markdown("~~strikethrough~~"));
        assert!(has_markdown("> blockquote"));
        assert!(has_markdown("[link](url)"));
        assert!(has_markdown("![image](url)"));
        assert!(has_markdown("- [ ] task"));
        assert!(has_markdown("- [x] completed"));
        assert!(has_markdown("- [X] completed"));
        assert!(has_markdown("| table | header |\n|-------|--------|"));
        assert!(has_markdown("---"));

        // Test plain text
        assert!(!has_markdown("Plain text without any formatting"));
        assert!(!has_markdown("Just some text with spaces"));
    }

    #[test]
    fn test_sample_markdown_generation() {
        let sample = create_sample_markdown();
        assert!(!sample.is_empty());
        assert!(has_markdown(&sample));
        assert!(sample.contains("# Markdown Rendering Demo"));
        assert!(sample.contains("```rust"));
        assert!(sample.contains("- [x]"));
        assert!(sample.contains("| Feature"));
    }
}
