# Enhanced Styling for Safe Coder

## Overview
This document shows you how to integrate TailwindCSS-inspired styling into your Rust TUI application.

## Quick Start

1. **Basic Usage in your existing UI**:
```rust
use crate::tui::{Theme, StyledComponents, LayoutUtils};

fn draw_ui(f: &mut Frame, app: &mut App) {
    let theme = Theme::dark(); // or Theme::light(), Theme::monokai()
    
    // Create a styled card
    let card = StyledComponents::card("My Component", &theme);
    
    // Use TailwindCSS-like colors
    let primary_color = theme.colors.primary; // Blue 500
    let success_color = theme.colors.success; // Green 500
}
```

2. **Enhanced UI Mode**:
```rust
use crate::tui::{draw_enhanced, Theme, ThemeManager};

// In your TUI runner
let theme = Theme::dark();
terminal.draw(|f| draw_enhanced(f, &mut app, &theme))?;
```

3. **Theme Management**:
```rust
use crate::tui::ThemeManager;

let mut theme_manager = ThemeManager::new(config_dir);
theme_manager.load().await?;

// Use throughout your app
let current_theme = theme_manager.get_current_theme();

// Switch themes
theme_manager.set_theme("monokai".to_string()).await?;
```

## Features

### 🎨 TailwindCSS Color System
- **Slate**: `TailwindColors::slate(500)` - Gray tones
- **Blue**: `TailwindColors::blue(500)` - Primary blue
- **Green**: `TailwindColors::green(500)` - Success green  
- **Red**: `TailwindColors::red(500)` - Error red
- **Yellow**: `TailwindColors::yellow(500)` - Warning yellow
- **Purple**: `TailwindColors::purple(500)` - Accent purple

### 🧩 Styled Components
```rust
// Cards
StyledComponents::card("Title", &theme)
StyledComponents::card_focused("Active", &theme)
StyledComponents::card_error("Error", &theme)

// Status badges
StyledComponents::status_badge("success", &theme)
StyledComponents::status_badge("error", &theme)
StyledComponents::status_badge("warning", &theme)

// Progress bars
StyledComponents::progress_bar(0.75, &theme) // 75% complete

// Code blocks
StyledComponents::code_block("fn main() {}", &theme)

// Modals
StyledComponents::modal("Settings", &theme)

// Notifications
StyledComponents::notification("Task completed!", "success", &theme)

// Gradient text
let spans = StyledComponents::gradient_text("Safe Coder", 400, 600);
```

### 📐 Layout Utilities
```rust
// Grid layout
let grid = LayoutUtils::grid_layout(area, 3, 2); // 3 cols, 2 rows

// Sidebar layout
let [sidebar, content] = LayoutUtils::sidebar_layout(area, 30);

// Page layout (header/content/footer)
let [header, content, footer] = LayoutUtils::page_layout(area, 3, 3);

// Centered modal
let modal_area = StyledComponents::centered_rect(60, 40, area);

// Card with padding
let padded_area = LayoutUtils::card_with_padding(area, 2);
```

## Themes

### Built-in Themes
1. **Dark Theme** - Modern dark mode with blue accents
2. **Light Theme** - Clean light mode with subtle shadows
3. **Monokai Theme** - Popular code editor color scheme

### Custom Themes
```rust
let custom_theme = Theme {
    name: "Custom".to_string(),
    colors: ColorPalette {
        primary: Color::Rgb(255, 0, 128),
        background: Color::Rgb(20, 20, 20),
        // ... customize all colors
    },
    styles: StyleSet {
        // ... customize all styles
    },
};

theme_manager.add_custom_theme(custom_theme).await?;
```

## Accessibility

### High Contrast Mode
```rust
theme_manager.toggle_high_contrast().await?;
let accessible_theme = theme_manager.get_current_theme(); // Auto-applies contrast
```

### Font Size Control
```rust
theme_manager.increase_font_size();
theme_manager.decrease_font_size();
theme_manager.reset_font_size();
```

## Advanced Features (Optional)

Enable with: `cargo build --features enhanced-styling`

```rust
#[cfg(feature = "enhanced-styling")]
use palette::{Hsv, Srgb};

// Color operations
let base = Srgb::new(0.23, 0.51, 0.96);
let variations = generate_color_scheme(base);
```

## Integration Examples

### Update your main UI function:
```rust
// src/tui/ui.rs - Replace existing draw function
pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = Theme::dark(); // Get from theme manager
    draw_enhanced(f, app, &theme);
}
```

### Add theme switching to keyboard handlers:
```rust
KeyCode::F1 => {
    let new_theme = app.theme_manager.cycle_theme().await?;
    app.set_status(&format!("Switched to {} theme", new_theme));
}
KeyCode::F2 => {
    app.theme_manager.toggle_high_contrast().await?;
    app.set_status("Toggled high contrast mode");
}
```

### Save themes in your config:
```rust
// src/config.rs - Add to your existing Config struct
pub struct Config {
    // ... existing fields
    pub styling: StylingConfig,
}
```

## Migration Guide

1. **Add the styling modules** to your `src/tui/mod.rs`
2. **Update Cargo.toml** with the enhanced ratatui features
3. **Replace your draw function** with the enhanced version
4. **Add theme management** to your app state
5. **Update keyboard handlers** for theme switching
6. **Customize colors and themes** to match your brand

## Color Reference

All colors follow TailwindCSS naming:
- 50: Lightest shade
- 100-400: Light shades  
- 500: Base color (recommended for primary use)
- 600-800: Dark shades
- 900-950: Darkest shades

Example: `TailwindColors::blue(500)` = RGB(59, 130, 246)

This styling system gives you modern, consistent, and accessible UI components while maintaining the performance benefits of a terminal-based interface!