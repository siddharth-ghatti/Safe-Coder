use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;

/// Memory/instruction management for the AI
pub struct MemoryManager {
    project_path: PathBuf,
    custom_instructions: Vec<String>,
}

impl MemoryManager {
    /// Create new memory manager
    pub fn new(project_path: PathBuf) -> Self {
        Self {
            project_path,
            custom_instructions: Vec::new(),
        }
    }

    /// Get memory file path
    fn memory_file_path(&self) -> PathBuf {
        self.project_path.join(".safe-coder").join("SAFE_CODER.md")
    }

    /// Load memory from SAFE_CODER.md file
    pub async fn load_from_file(&mut self) -> Result<String> {
        let memory_path = self.memory_file_path();

        if !memory_path.exists() {
            return Ok(String::new());
        }

        let content = fs::read_to_string(&memory_path)
            .await
            .context("Failed to read SAFE_CODER.md")?;

        Ok(content)
    }

    /// Add custom instruction
    pub fn add_instruction(&mut self, instruction: String) {
        self.custom_instructions.push(instruction);
    }

    /// Get all instructions as system prompt
    pub async fn get_system_prompt(&mut self) -> Result<String> {
        let mut prompt = String::new();

        // Load from file
        let file_content = self.load_from_file().await?;
        if !file_content.is_empty() {
            prompt.push_str(&file_content);
            prompt.push_str("\n\n");
        }

        // Add custom instructions
        if !self.custom_instructions.is_empty() {
            prompt.push_str("Additional Instructions:\n");
            for instruction in &self.custom_instructions {
                prompt.push_str("- ");
                prompt.push_str(instruction);
                prompt.push_str("\n");
            }
        }

        Ok(prompt)
    }

    /// Show current memory
    pub async fn show(&self) -> Result<String> {
        let mut output = String::new();
        output.push_str("💭 Memory & Instructions\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        // File-based memory
        let memory_path = self.memory_file_path();
        if memory_path.exists() {
            output.push_str(&format!("📄 From SAFE_CODER.md ({})\n\n", memory_path.display()));
            let content = fs::read_to_string(&memory_path).await?;
            output.push_str(&content);
            output.push_str("\n\n");
        } else {
            output.push_str("📄 No SAFE_CODER.md file found\n\n");
        }

        // Custom instructions
        if !self.custom_instructions.is_empty() {
            output.push_str("📝 Custom Instructions:\n");
            for (i, instruction) in self.custom_instructions.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, instruction));
            }
        } else {
            output.push_str("📝 No custom instructions added\n");
        }

        Ok(output)
    }

    /// Refresh from file (discard custom instructions)
    pub async fn refresh(&mut self) -> Result<()> {
        self.custom_instructions.clear();
        Ok(())
    }

    /// Create default SAFE_CODER.md file
    pub async fn init_file(&self) -> Result<()> {
        let memory_path = self.memory_file_path();

        // Create .safe-coder directory
        if let Some(parent) = memory_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let default_content = r#"# Project Context for Safe Coder

This file contains instructions and context that will be included in every conversation with Safe Coder AI.

## Project Overview

[Describe your project here]

## Code Style Guidelines

[Add your coding standards and preferences]

## Important Conventions

[List any project-specific conventions the AI should follow]

## Files to Focus On

[List important files or directories]

## Things to Avoid

[List any patterns or practices to avoid]
"#;

        fs::write(&memory_path, default_content).await?;

        Ok(())
    }

    /// Clear custom instructions
    pub fn clear_custom(&mut self) {
        self.custom_instructions.clear();
    }
}
