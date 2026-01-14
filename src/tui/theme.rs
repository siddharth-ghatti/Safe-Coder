use ratatui::style::{Color, Modifier, Style};


#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub colors: ColorPalette,
    pub styles: StyleSet,
}

#[derive(Debug, Clone)]
pub struct ColorPalette {
    // Primary colors
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    
    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    
    // Surface colors
    pub background: Color,
    pub surface: Color,
    pub surface_variant: Color,
    
    // Text colors
    pub on_background: Color,
    pub on_surface: Color,
    pub on_primary: Color,
    
    // Border colors
    pub border: Color,
    pub border_focus: Color,
    pub border_error: Color,
}

#[derive(Debug, Clone)]
pub struct StyleSet {
    pub normal: Style,
    pub focused: Style,
    pub selected: Style,
    pub error: Style,
    pub success: Style,
    pub warning: Style,
    pub info: Style,
}

impl Theme {
    pub fn dark() -> Self {
        let colors = ColorPalette {
            primary: Color::Rgb(59, 130, 246),      // Blue 500
            secondary: Color::Rgb(107, 114, 128),   // Gray 500
            accent: Color::Rgb(168, 85, 247),       // Purple 500
            
            success: Color::Rgb(34, 197, 94),       // Green 500
            warning: Color::Rgb(251, 191, 36),      // Yellow 500
            error: Color::Rgb(239, 68, 68),         // Red 500
            info: Color::Rgb(59, 130, 246),         // Blue 500
            
            background: Color::Rgb(17, 24, 39),     // Gray 900
            surface: Color::Rgb(31, 41, 55),        // Gray 800
            surface_variant: Color::Rgb(55, 65, 81), // Gray 700
            
            on_background: Color::Rgb(243, 244, 246), // Gray 100
            on_surface: Color::Rgb(229, 231, 235),   // Gray 200
            on_primary: Color::Rgb(255, 255, 255),   // White
            
            border: Color::Rgb(75, 85, 99),         // Gray 600
            border_focus: Color::Rgb(59, 130, 246), // Blue 500
            border_error: Color::Rgb(239, 68, 68),  // Red 500
        };

        let styles = StyleSet {
            normal: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface),
            focused: Style::default()
                .fg(colors.on_primary)
                .bg(colors.primary)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface_variant)
                .add_modifier(Modifier::REVERSED),
            error: Style::default()
                .fg(colors.error)
                .add_modifier(Modifier::BOLD),
            success: Style::default()
                .fg(colors.success)
                .add_modifier(Modifier::BOLD),
            warning: Style::default()
                .fg(colors.warning)
                .add_modifier(Modifier::BOLD),
            info: Style::default()
                .fg(colors.info)
                .add_modifier(Modifier::BOLD),
        };

        Theme {
            name: "Dark".to_string(),
            colors,
            styles,
        }
    }

    pub fn light() -> Self {
        let colors = ColorPalette {
            primary: Color::Rgb(59, 130, 246),      // Blue 500
            secondary: Color::Rgb(107, 114, 128),   // Gray 500
            accent: Color::Rgb(168, 85, 247),       // Purple 500
            
            success: Color::Rgb(34, 197, 94),       // Green 500
            warning: Color::Rgb(251, 191, 36),      // Yellow 500
            error: Color::Rgb(239, 68, 68),         // Red 500
            info: Color::Rgb(59, 130, 246),         // Blue 500
            
            background: Color::Rgb(255, 255, 255),  // White
            surface: Color::Rgb(249, 250, 251),     // Gray 50
            surface_variant: Color::Rgb(243, 244, 246), // Gray 100
            
            on_background: Color::Rgb(17, 24, 39),  // Gray 900
            on_surface: Color::Rgb(31, 41, 55),     // Gray 800
            on_primary: Color::Rgb(255, 255, 255),  // White
            
            border: Color::Rgb(209, 213, 219),      // Gray 300
            border_focus: Color::Rgb(59, 130, 246), // Blue 500
            border_error: Color::Rgb(239, 68, 68),  // Red 500
        };

        let styles = StyleSet {
            normal: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface),
            focused: Style::default()
                .fg(colors.on_primary)
                .bg(colors.primary)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface_variant)
                .add_modifier(Modifier::REVERSED),
            error: Style::default()
                .fg(colors.error)
                .add_modifier(Modifier::BOLD),
            success: Style::default()
                .fg(colors.success)
                .add_modifier(Modifier::BOLD),
            warning: Style::default()
                .fg(colors.warning)
                .add_modifier(Modifier::BOLD),
            info: Style::default()
                .fg(colors.info)
                .add_modifier(Modifier::BOLD),
        };

        Theme {
            name: "Light".to_string(),
            colors,
            styles,
        }
    }

    pub fn monokai() -> Self {
        let colors = ColorPalette {
            primary: Color::Rgb(166, 226, 46),      // Monokai green
            secondary: Color::Rgb(174, 129, 255),   // Monokai purple
            accent: Color::Rgb(255, 216, 102),      // Monokai yellow
            
            success: Color::Rgb(166, 226, 46),      // Green
            warning: Color::Rgb(255, 216, 102),     // Yellow
            error: Color::Rgb(249, 38, 114),        // Pink
            info: Color::Rgb(102, 217, 239),        // Cyan
            
            background: Color::Rgb(39, 40, 34),     // Dark bg
            surface: Color::Rgb(73, 72, 62),        // Lighter dark
            surface_variant: Color::Rgb(90, 89, 82), // Even lighter
            
            on_background: Color::Rgb(248, 248, 242), // Light text
            on_surface: Color::Rgb(248, 248, 242),   // Light text
            on_primary: Color::Rgb(39, 40, 34),     // Dark text on primary
            
            border: Color::Rgb(90, 89, 82),         // Muted border
            border_focus: Color::Rgb(166, 226, 46), // Green focus
            border_error: Color::Rgb(249, 38, 114), // Pink error
        };

        let styles = StyleSet {
            normal: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface),
            focused: Style::default()
                .fg(colors.on_primary)
                .bg(colors.primary)
                .add_modifier(Modifier::BOLD),
            selected: Style::default()
                .fg(colors.on_surface)
                .bg(colors.surface_variant)
                .add_modifier(Modifier::REVERSED),
            error: Style::default()
                .fg(colors.error)
                .add_modifier(Modifier::BOLD),
            success: Style::default()
                .fg(colors.success)
                .add_modifier(Modifier::BOLD),
            warning: Style::default()
                .fg(colors.warning)
                .add_modifier(Modifier::BOLD),
            info: Style::default()
                .fg(colors.info)
                .add_modifier(Modifier::BOLD),
        };

        Theme {
            name: "Monokai".to_string(),
            colors,
            styles,
        }
    }
}

// TailwindCSS-inspired utility functions
pub struct TailwindColors;

impl TailwindColors {
    pub fn slate(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(248, 250, 252),
            100 => Color::Rgb(241, 245, 249),
            200 => Color::Rgb(226, 232, 240),
            300 => Color::Rgb(203, 213, 225),
            400 => Color::Rgb(148, 163, 184),
            500 => Color::Rgb(100, 116, 139),
            600 => Color::Rgb(71, 85, 105),
            700 => Color::Rgb(51, 65, 85),
            800 => Color::Rgb(30, 41, 59),
            900 => Color::Rgb(15, 23, 42),
            950 => Color::Rgb(2, 6, 23),
            _ => Color::Gray,
        }
    }

    pub fn blue(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(239, 246, 255),
            100 => Color::Rgb(219, 234, 254),
            200 => Color::Rgb(191, 219, 254),
            300 => Color::Rgb(147, 197, 253),
            400 => Color::Rgb(96, 165, 250),
            500 => Color::Rgb(59, 130, 246),
            600 => Color::Rgb(37, 99, 235),
            700 => Color::Rgb(29, 78, 216),
            800 => Color::Rgb(30, 64, 175),
            900 => Color::Rgb(30, 58, 138),
            950 => Color::Rgb(23, 37, 84),
            _ => Color::Blue,
        }
    }

    pub fn green(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(240, 253, 244),
            100 => Color::Rgb(220, 252, 231),
            200 => Color::Rgb(187, 247, 208),
            300 => Color::Rgb(134, 239, 172),
            400 => Color::Rgb(74, 222, 128),
            500 => Color::Rgb(34, 197, 94),
            600 => Color::Rgb(22, 163, 74),
            700 => Color::Rgb(21, 128, 61),
            800 => Color::Rgb(22, 101, 52),
            900 => Color::Rgb(20, 83, 45),
            950 => Color::Rgb(5, 46, 22),
            _ => Color::Green,
        }
    }

    pub fn red(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(254, 242, 242),
            100 => Color::Rgb(254, 226, 226),
            200 => Color::Rgb(254, 202, 202),
            300 => Color::Rgb(252, 165, 165),
            400 => Color::Rgb(248, 113, 113),
            500 => Color::Rgb(239, 68, 68),
            600 => Color::Rgb(220, 38, 38),
            700 => Color::Rgb(185, 28, 28),
            800 => Color::Rgb(153, 27, 27),
            900 => Color::Rgb(127, 29, 29),
            950 => Color::Rgb(69, 10, 10),
            _ => Color::Red,
        }
    }

    pub fn yellow(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(254, 252, 232),
            100 => Color::Rgb(254, 249, 195),
            200 => Color::Rgb(254, 240, 138),
            300 => Color::Rgb(253, 224, 71),
            400 => Color::Rgb(250, 204, 21),
            500 => Color::Rgb(234, 179, 8),
            600 => Color::Rgb(202, 138, 4),
            700 => Color::Rgb(161, 98, 7),
            800 => Color::Rgb(133, 77, 14),
            900 => Color::Rgb(113, 63, 18),
            950 => Color::Rgb(66, 32, 6),
            _ => Color::Yellow,
        }
    }

    pub fn purple(shade: u16) -> Color {
        match shade {
            50 => Color::Rgb(250, 245, 255),
            100 => Color::Rgb(243, 232, 255),
            200 => Color::Rgb(233, 213, 255),
            300 => Color::Rgb(196, 181, 253),
            400 => Color::Rgb(168, 85, 247),
            500 => Color::Rgb(147, 51, 234),
            600 => Color::Rgb(126, 34, 206),
            700 => Color::Rgb(107, 33, 168),
            800 => Color::Rgb(88, 28, 135),
            900 => Color::Rgb(74, 29, 114),
            950 => Color::Rgb(59, 7, 100),
            _ => Color::Magenta,
        }
    }
}