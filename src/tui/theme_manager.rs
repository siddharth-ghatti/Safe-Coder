use super::theme::Theme;
use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;

#[derive(Debug, Clone)]
pub struct StylingConfig {
    pub current_theme: String,
    pub custom_themes: Vec<Theme>,
    pub animations_enabled: bool,
    pub high_contrast: bool,
    pub font_size_modifier: i8, // -2 to +2
}

impl Default for StylingConfig {
    fn default() -> Self {
        Self {
            current_theme: "dark".to_string(),
            custom_themes: Vec::new(),
            animations_enabled: true,
            high_contrast: false,
            font_size_modifier: 0,
        }
    }
}

impl StylingConfig {
    /// Load styling configuration from file
    pub async fn load_from_file(_path: &PathBuf) -> Result<Self> {
        // For now, just return default config
        // TODO: Implement proper config loading without serde
        Ok(Self::default())
    }

    /// Save styling configuration to file
    pub async fn save_to_file(&self, _path: &PathBuf) -> Result<()> {
        // For now, do nothing
        // TODO: Implement proper config saving without serde
        Ok(())
    }

    /// Get the current theme
    pub fn get_current_theme(&self) -> Theme {
        // First check custom themes
        for theme in &self.custom_themes {
            if theme.name == self.current_theme {
                return theme.clone();
            }
        }

        // Fall back to built-in themes
        match self.current_theme.as_str() {
            "light" => Theme::light(),
            "monokai" => Theme::monokai(),
            _ => Theme::dark(),
        }
    }

    /// Set the current theme
    pub fn set_theme(&mut self, theme_name: String) {
        self.current_theme = theme_name;
    }

    /// Add a custom theme
    pub fn add_custom_theme(&mut self, theme: Theme) {
        // Remove existing theme with same name
        self.custom_themes.retain(|t| t.name != theme.name);
        self.custom_themes.push(theme);
    }

    /// List all available themes
    pub fn list_available_themes(&self) -> Vec<String> {
        let mut themes = vec!["dark".to_string(), "light".to_string(), "monokai".to_string()];
        
        for theme in &self.custom_themes {
            themes.push(theme.name.clone());
        }

        themes.sort();
        themes
    }

    /// Toggle high contrast mode
    pub fn toggle_high_contrast(&mut self) {
        self.high_contrast = !self.high_contrast;
    }

    /// Increase font size
    pub fn increase_font_size(&mut self) {
        if self.font_size_modifier < 2 {
            self.font_size_modifier += 1;
        }
    }

    /// Decrease font size
    pub fn decrease_font_size(&mut self) {
        if self.font_size_modifier > -2 {
            self.font_size_modifier -= 1;
        }
    }

    /// Reset font size to default
    pub fn reset_font_size(&mut self) {
        self.font_size_modifier = 0;
    }

    /// Get modified theme with accessibility settings applied
    pub fn get_accessible_theme(&self) -> Theme {
        let mut theme = self.get_current_theme();

        if self.high_contrast {
            // Modify colors for higher contrast
            theme.colors.on_background = ratatui::style::Color::White;
            theme.colors.on_surface = ratatui::style::Color::White;
            theme.colors.background = ratatui::style::Color::Black;
            theme.colors.surface = ratatui::style::Color::Rgb(20, 20, 20);
        }

        theme
    }
}

/// Theme manager for handling theme operations
pub struct ThemeManager {
    config: StylingConfig,
    config_path: PathBuf,
}

impl ThemeManager {
    pub fn new(config_dir: PathBuf) -> Self {
        let config_path = config_dir.join("styling.toml");
        Self {
            config: StylingConfig::default(),
            config_path,
        }
    }

    pub async fn load(&mut self) -> Result<()> {
        self.config = StylingConfig::load_from_file(&self.config_path).await?;
        Ok(())
    }

    pub async fn save(&self) -> Result<()> {
        self.config.save_to_file(&self.config_path).await?;
        Ok(())
    }

    pub fn get_current_theme(&self) -> Theme {
        self.config.get_accessible_theme()
    }

    pub async fn set_theme(&mut self, theme_name: String) -> Result<()> {
        self.config.set_theme(theme_name);
        self.save().await?;
        Ok(())
    }

    pub async fn add_custom_theme(&mut self, theme: Theme) -> Result<()> {
        self.config.add_custom_theme(theme);
        self.save().await?;
        Ok(())
    }

    pub fn list_themes(&self) -> Vec<String> {
        self.config.list_available_themes()
    }

    pub async fn toggle_high_contrast(&mut self) -> Result<()> {
        self.config.toggle_high_contrast();
        self.save().await?;
        Ok(())
    }

    pub fn is_high_contrast(&self) -> bool {
        self.config.high_contrast
    }

    pub async fn cycle_theme(&mut self) -> Result<String> {
        let themes = self.list_themes();
        let current_index = themes
            .iter()
            .position(|t| t == &self.config.current_theme)
            .unwrap_or(0);
        
        let next_index = (current_index + 1) % themes.len();
        let next_theme = themes[next_index].clone();
        
        self.set_theme(next_theme.clone()).await?;
        Ok(next_theme)
    }
    
    /// Synchronous theme cycling for keyboard shortcuts (doesn't persist)
    pub fn cycle_theme_sync(&mut self) {
        let themes = self.list_themes();
        let current_index = themes
            .iter()
            .position(|t| t == &self.config.current_theme)
            .unwrap_or(0);
        
        let next_index = (current_index + 1) % themes.len();
        let next_theme = themes[next_index].clone();
        
        self.config.set_theme(next_theme);
    }
    
    /// Get current theme name
    pub fn current_theme_name(&self) -> &str {
        &self.config.current_theme
    }
}