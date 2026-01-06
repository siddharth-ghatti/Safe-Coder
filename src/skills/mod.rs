//! Skills System
//!
//! Skills are loadable knowledge modules that inject specialized context
//! into AI conversations. They can be triggered by file patterns or
//! explicitly loaded via the /skill command.
//!
//! Skills are markdown files with optional YAML frontmatter:
//! ```markdown
//! ---
//! name: react-patterns
//! trigger: "*.tsx"
//! description: React best practices and patterns
//! ---
//!
//! # React Best Practices
//!
//! When working with React components:
//! 1. Use functional components with hooks
//! ...
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A loaded skill with its content and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Unique name of the skill
    pub name: String,
    /// Description of what this skill provides
    pub description: Option<String>,
    /// File patterns that trigger this skill (e.g., "*.tsx", "Dockerfile")
    pub triggers: Vec<String>,
    /// The skill content (markdown)
    pub content: String,
    /// Source file path
    pub source_path: Option<PathBuf>,
    /// Whether this skill is currently active
    pub active: bool,
}

impl Skill {
    /// Create a new skill from content with frontmatter
    pub fn from_content(content: &str, source_path: Option<PathBuf>) -> Result<Self> {
        let (frontmatter, body) = parse_frontmatter(content)?;

        let name = frontmatter
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                source_path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });

        let description = frontmatter
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let triggers: Vec<String> = frontmatter
            .get("trigger")
            .or_else(|| frontmatter.get("triggers"))
            .map(|v| match v {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();

        Ok(Self {
            name,
            description,
            triggers,
            content: body,
            source_path,
            active: false,
        })
    }

    /// Check if this skill should be triggered by a file path
    pub fn matches_file(&self, file_path: &str) -> bool {
        for trigger in &self.triggers {
            if matches_pattern(trigger, file_path) {
                return true;
            }
        }
        false
    }

    /// Get the content formatted for injection into a prompt
    pub fn to_prompt_injection(&self) -> String {
        let mut result = format!("# Skill: {}\n\n", self.name);
        if let Some(ref desc) = self.description {
            result.push_str(&format!("*{}*\n\n", desc));
        }
        result.push_str(&self.content);
        result
    }
}

/// Skill manager that handles loading and activation of skills
pub struct SkillManager {
    /// All loaded skills
    skills: HashMap<String, Skill>,
    /// Search paths for skill files
    search_paths: Vec<PathBuf>,
}

impl SkillManager {
    /// Create a new skill manager
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            search_paths: Vec::new(),
        }
    }

    /// Add a search path for skill files
    pub fn add_search_path(&mut self, path: PathBuf) {
        if !self.search_paths.contains(&path) {
            self.search_paths.push(path);
        }
    }

    /// Initialize with default search paths for a project
    pub fn with_project_paths(project_path: &Path) -> Self {
        let mut manager = Self::new();

        // Add project-level skills directory
        manager.add_search_path(project_path.join(".safe-coder").join("skills"));

        // Add user-level skills directory
        if let Some(config_dir) = dirs::config_dir() {
            manager.add_search_path(config_dir.join("safe-coder").join("skills"));
        }

        manager
    }

    /// Load all skills from search paths
    pub async fn load_all(&mut self) -> Result<usize> {
        let mut loaded = 0;

        for search_path in self.search_paths.clone() {
            if search_path.exists() && search_path.is_dir() {
                loaded += self.load_from_directory(&search_path).await?;
            }
        }

        Ok(loaded)
    }

    /// Load skills from a directory
    pub async fn load_from_directory(&mut self, dir: &Path) -> Result<usize> {
        let mut loaded = 0;

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(0),
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "md" || ext == "markdown" {
                        if let Ok(skill) = self.load_skill_file(&path).await {
                            self.skills.insert(skill.name.clone(), skill);
                            loaded += 1;
                        }
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Load a single skill file
    pub async fn load_skill_file(&self, path: &Path) -> Result<Skill> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read skill file")?;

        Skill::from_content(&content, Some(path.to_path_buf()))
    }

    /// Get a skill by name
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Activate a skill by name
    pub fn activate(&mut self, name: &str) -> bool {
        if let Some(skill) = self.skills.get_mut(name) {
            skill.active = true;
            true
        } else {
            false
        }
    }

    /// Deactivate a skill by name
    pub fn deactivate(&mut self, name: &str) -> bool {
        if let Some(skill) = self.skills.get_mut(name) {
            skill.active = false;
            true
        } else {
            false
        }
    }

    /// Get all active skills
    pub fn get_active(&self) -> Vec<&Skill> {
        self.skills.values().filter(|s| s.active).collect()
    }

    /// Get skills that match a file path
    pub fn get_matching(&self, file_path: &str) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.matches_file(file_path))
            .collect()
    }

    /// Auto-activate skills based on files being worked on
    pub fn auto_activate_for_files(&mut self, file_paths: &[&str]) {
        for (_, skill) in self.skills.iter_mut() {
            for file_path in file_paths {
                if skill.matches_file(file_path) {
                    skill.active = true;
                    break;
                }
            }
        }
    }

    /// List all available skills
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// Get combined content of all active skills for prompt injection
    pub fn get_active_skills_prompt(&self) -> Option<String> {
        let active: Vec<_> = self.get_active();
        if active.is_empty() {
            return None;
        }

        let mut content = String::from("\n---\n# Active Skills\n\n");
        for skill in active {
            content.push_str(&skill.to_prompt_injection());
            content.push_str("\n\n---\n\n");
        }

        Some(content)
    }

    /// Register a skill directly (for built-in skills)
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str) -> Result<(serde_json::Value, String)> {
    let content = content.trim();

    if !content.starts_with("---") {
        // No frontmatter, return empty object and full content
        return Ok((
            serde_json::Value::Object(Default::default()),
            content.to_string(),
        ));
    }

    // Find the closing ---
    let rest = &content[3..];
    let end_pos = rest.find("\n---");

    match end_pos {
        Some(pos) => {
            let yaml_content = &rest[..pos].trim();
            let body = &rest[pos + 4..].trim();

            // Parse YAML as JSON (serde_yaml would be cleaner but adds a dep)
            // Simple key: value parsing
            let mut obj = serde_json::Map::new();
            for line in yaml_content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim().to_string();
                    let value = line[colon_pos + 1..].trim();

                    // Handle arrays (simple format: [a, b, c])
                    let json_value = if value.starts_with('[') && value.ends_with(']') {
                        let inner = &value[1..value.len() - 1];
                        let items: Vec<serde_json::Value> = inner
                            .split(',')
                            .map(|s| {
                                serde_json::Value::String(s.trim().trim_matches('"').to_string())
                            })
                            .collect();
                        serde_json::Value::Array(items)
                    } else {
                        serde_json::Value::String(value.trim_matches('"').to_string())
                    };

                    obj.insert(key, json_value);
                }
            }

            Ok((serde_json::Value::Object(obj), body.to_string()))
        }
        None => {
            // No closing ---, treat as no frontmatter
            Ok((
                serde_json::Value::Object(Default::default()),
                content.to_string(),
            ))
        }
    }
}

/// Simple pattern matching (supports * and **)
fn matches_pattern(pattern: &str, path: &str) -> bool {
    // Normalize paths
    let path = path.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");

    // Handle exact match
    if pattern == path {
        return true;
    }

    // Handle *.ext patterns
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // includes the dot
        return path.ends_with(ext);
    }

    // Handle **/name patterns (match filename anywhere)
    if pattern.starts_with("**/") {
        let name = &pattern[3..];
        return path.ends_with(name) || path.contains(&format!("/{}", name));
    }

    // Handle dir/** patterns (match anything under dir)
    if pattern.ends_with("/**") {
        let prefix = &pattern[..pattern.len() - 3];
        return path.starts_with(prefix);
    }

    // Handle simple * wildcard
    if pattern.contains('*') && !pattern.contains("**") {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return path.starts_with(parts[0]) && path.ends_with(parts[1]);
        }
    }

    false
}

/// Built-in skills that are always available
pub mod builtin {
    use super::Skill;

    /// Rust best practices skill
    pub fn rust_skill() -> Skill {
        Skill {
            name: "rust-patterns".to_string(),
            description: Some("Rust idioms and best practices".to_string()),
            triggers: vec!["*.rs".to_string()],
            content: r#"
## Rust Best Practices

When writing Rust code:

1. **Error Handling**: Use `Result<T, E>` for recoverable errors, `panic!` only for unrecoverable bugs
2. **Ownership**: Prefer borrowing (`&T`, `&mut T`) over cloning when possible
3. **Iterators**: Use iterator methods instead of manual loops
4. **Pattern Matching**: Use `match` and `if let` for exhaustive handling
5. **Documentation**: Add doc comments (`///`) for public items
6. **Testing**: Include unit tests in the same file with `#[cfg(test)]` module
7. **Error Types**: Use `thiserror` for library errors, `anyhow` for application errors
8. **Async**: Prefer `tokio` for async runtime, use `async-trait` for async traits
"#.to_string(),
            source_path: None,
            active: false,
        }
    }

    /// React/TypeScript skill
    pub fn react_skill() -> Skill {
        Skill {
            name: "react-patterns".to_string(),
            description: Some("React and TypeScript best practices".to_string()),
            triggers: vec!["*.tsx".to_string(), "*.jsx".to_string()],
            content: r#"
## React Best Practices

When writing React components:

1. **Functional Components**: Use functional components with hooks instead of class components
2. **TypeScript**: Always define proper types for props and state
3. **Hooks**: Follow the rules of hooks (only call at top level, only in React functions)
4. **State Management**: Use `useState` for local state, context or state libraries for global
5. **Memoization**: Use `useMemo` and `useCallback` to prevent unnecessary re-renders
6. **Effects**: Keep `useEffect` dependencies accurate, clean up subscriptions
7. **Component Structure**: Keep components small and focused
8. **Keys**: Always use unique keys for lists, avoid using array indices
"#
            .to_string(),
            source_path: None,
            active: false,
        }
    }

    /// Python skill
    pub fn python_skill() -> Skill {
        Skill {
            name: "python-patterns".to_string(),
            description: Some("Python idioms and best practices".to_string()),
            triggers: vec!["*.py".to_string()],
            content: r#"
## Python Best Practices

When writing Python code:

1. **Type Hints**: Use type annotations for function signatures
2. **Docstrings**: Document functions with docstrings (Google or NumPy style)
3. **Virtual Environments**: Use venv or poetry for dependency management
4. **List Comprehensions**: Prefer comprehensions over map/filter when readable
5. **Context Managers**: Use `with` statements for resource management
6. **F-Strings**: Use f-strings for string formatting (Python 3.6+)
7. **Dataclasses**: Use `@dataclass` for simple data containers
8. **Pathlib**: Use `pathlib.Path` instead of `os.path` for file paths
"#
            .to_string(),
            source_path: None,
            active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: test-skill
trigger: "*.rs"
description: A test skill
---

# Test Content

This is the body.
"#;

        let (fm, body) = parse_frontmatter(content).unwrap();
        assert_eq!(fm["name"], "test-skill");
        assert_eq!(fm["trigger"], "*.rs");
        assert!(body.contains("# Test Content"));
    }

    #[test]
    fn test_skill_from_content() {
        let content = r#"---
name: my-skill
trigger: ["*.ts", "*.tsx"]
---

# My Skill

Content here.
"#;

        let skill = Skill::from_content(content, None).unwrap();
        assert_eq!(skill.name, "my-skill");
        assert_eq!(skill.triggers.len(), 2);
    }

    #[test]
    fn test_pattern_matching() {
        assert!(matches_pattern("*.rs", "src/main.rs"));
        assert!(matches_pattern("*.rs", "lib.rs"));
        assert!(!matches_pattern("*.rs", "main.py"));

        assert!(matches_pattern("**/mod.rs", "src/tools/mod.rs"));
        assert!(matches_pattern("**/mod.rs", "mod.rs"));

        assert!(matches_pattern("src/**", "src/main.rs"));
        assert!(matches_pattern("src/**", "src/tools/mod.rs"));
    }

    #[test]
    fn test_skill_matches_file() {
        let skill = Skill {
            name: "test".to_string(),
            description: None,
            triggers: vec!["*.rs".to_string(), "*.toml".to_string()],
            content: String::new(),
            source_path: None,
            active: false,
        };

        assert!(skill.matches_file("src/main.rs"));
        assert!(skill.matches_file("Cargo.toml"));
        assert!(!skill.matches_file("package.json"));
    }
}
