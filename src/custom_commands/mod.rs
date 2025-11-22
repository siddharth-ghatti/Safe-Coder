use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Custom user-defined commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCommand {
    pub name: String,
    pub description: Option<String>,
    pub prompt: String,
}

/// Custom command manager
pub struct CustomCommandManager {
    project_path: PathBuf,
    global_commands: HashMap<String, CustomCommand>,
    project_commands: HashMap<String, CustomCommand>,
}

impl CustomCommandManager {
    /// Create new custom command manager
    pub async fn new(project_path: PathBuf) -> Result<Self> {
        let mut manager = Self {
            project_path,
            global_commands: HashMap::new(),
            project_commands: HashMap::new(),
        };

        manager.load_commands().await?;

        Ok(manager)
    }

    /// Load commands from global and project directories
    async fn load_commands(&mut self) -> Result<()> {
        // Load global commands from ~/.config/safe-coder/commands/
        if let Some(config_dir) = dirs::config_dir() {
            let global_dir = config_dir.join("safe-coder").join("commands");
            if global_dir.exists() {
                self.global_commands = Self::load_from_directory(&global_dir).await?;
            }
        }

        // Load project commands from .safe-coder/commands/
        let project_dir = self.project_path.join(".safe-coder").join("commands");
        if project_dir.exists() {
            self.project_commands = Self::load_from_directory(&project_dir).await?;
        }

        Ok(())
    }

    /// Load commands from a directory
    async fn load_from_directory(dir: &PathBuf) -> Result<HashMap<String, CustomCommand>> {
        let mut commands = HashMap::new();

        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                match Self::load_command_file(&path).await {
                    Ok(cmd) => {
                        commands.insert(cmd.name.clone(), cmd);
                    },
                    Err(e) => {
                        tracing::warn!("Failed to load command from {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(commands)
    }

    /// Load a single command file
    async fn load_command_file(path: &PathBuf) -> Result<CustomCommand> {
        let content = fs::read_to_string(path)
            .await
            .context("Failed to read command file")?;

        let value: toml::Value = toml::from_str(&content)
            .context("Failed to parse TOML")?;

        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .context("Invalid filename")?
            .to_string();

        let description = value.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt = value.get("prompt")
            .and_then(|v| v.as_str())
            .context("Command must have a 'prompt' field")?
            .to_string();

        Ok(CustomCommand {
            name,
            description,
            prompt,
        })
    }

    /// Get a command by name (project commands override global)
    pub fn get_command(&self, name: &str) -> Option<&CustomCommand> {
        self.project_commands
            .get(name)
            .or_else(|| self.global_commands.get(name))
    }

    /// List all available commands
    pub fn list_commands(&self) -> Vec<&CustomCommand> {
        let mut commands: Vec<&CustomCommand> = self.global_commands.values().collect();
        commands.extend(self.project_commands.values());
        commands
    }

    /// Execute a custom command by replacing {{args}} with provided arguments
    pub fn execute_command(&self, name: &str, args: &str) -> Result<String> {
        let cmd = self.get_command(name)
            .context("Command not found")?;

        // Replace {{args}} with actual arguments
        let prompt = cmd.prompt.replace("{{args}}", args);

        // TODO: Support !{...} for shell execution
        // TODO: Support @{...} for file inclusion

        Ok(prompt)
    }

    /// Reload commands from disk
    pub async fn reload(&mut self) -> Result<()> {
        self.global_commands.clear();
        self.project_commands.clear();
        self.load_commands().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_custom_command() {
        let temp_dir = TempDir::new().unwrap();
        let commands_dir = temp_dir.path().join("commands");
        fs::create_dir_all(&commands_dir).await.unwrap();

        // Create a test command file
        let cmd_file = commands_dir.join("test.toml");
        let content = r#"
description = "Test command"
prompt = "Do something with {{args}}"
"#;
        fs::write(&cmd_file, content).await.unwrap();

        // Load commands
        let commands = CustomCommandManager::load_from_directory(&commands_dir).await.unwrap();

        assert_eq!(commands.len(), 1);
        let cmd = commands.get("test").unwrap();
        assert_eq!(cmd.name, "test");
        assert_eq!(cmd.prompt, "Do something with {{args}}");
    }
}
