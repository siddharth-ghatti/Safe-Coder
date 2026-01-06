//! Integration tests for the Skills system
//!
//! Tests the SkillManager, skill loading, and pattern matching functionality.

use anyhow::Result;
use serial_test::serial;
use safe_coder::skills::{Skill, SkillManager, builtin};
use assert_fs::prelude::*;
use assert_fs::TempDir;
use std::path::PathBuf;

#[tokio::test]
#[serial]
async fn test_skill_manager_creation() -> Result<()> {
    let manager = SkillManager::new();
    let skills = manager.list();

    // New manager should have no skills
    assert!(skills.is_empty());

    Ok(())
}

#[test]
fn test_skill_from_content_with_frontmatter() -> Result<()> {
    let content = r#"---
name: test-skill
trigger: "*.rs"
description: A test skill for Rust files
---

# Test Skill Content

This is the skill body content.
"#;

    let skill = Skill::from_content(content, None)?;

    assert_eq!(skill.name, "test-skill");
    assert_eq!(skill.description, Some("A test skill for Rust files".to_string()));
    assert_eq!(skill.triggers, vec!["*.rs".to_string()]);
    assert!(skill.content.contains("Test Skill Content"));

    Ok(())
}

#[test]
fn test_skill_from_content_with_array_triggers() -> Result<()> {
    let content = r#"---
name: multi-trigger-skill
trigger: ["*.ts", "*.tsx", "*.js"]
description: Multi-language skill
---

# Content
"#;

    let skill = Skill::from_content(content, None)?;

    assert_eq!(skill.name, "multi-trigger-skill");
    assert_eq!(skill.triggers.len(), 3);
    assert!(skill.triggers.contains(&"*.ts".to_string()));
    assert!(skill.triggers.contains(&"*.tsx".to_string()));
    assert!(skill.triggers.contains(&"*.js".to_string()));

    Ok(())
}

#[test]
fn test_skill_from_content_no_frontmatter() -> Result<()> {
    let content = "# Just Some Content\n\nNo frontmatter here.";

    let skill = Skill::from_content(content, Some(PathBuf::from("my-skill.md")))?;

    // Should derive name from file path
    assert_eq!(skill.name, "my-skill");
    assert!(skill.triggers.is_empty());
    assert!(skill.content.contains("Just Some Content"));

    Ok(())
}

#[test]
fn test_skill_matches_file_extension() -> Result<()> {
    let skill = Skill {
        name: "rust-skill".to_string(),
        description: None,
        triggers: vec!["*.rs".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    };

    assert!(skill.matches_file("src/main.rs"));
    assert!(skill.matches_file("lib.rs"));
    assert!(skill.matches_file("deep/nested/file.rs"));
    assert!(!skill.matches_file("main.py"));
    assert!(!skill.matches_file("main.rst")); // Close but not a match

    Ok(())
}

#[test]
fn test_skill_matches_file_double_star() -> Result<()> {
    let skill = Skill {
        name: "mod-skill".to_string(),
        description: None,
        triggers: vec!["**/mod.rs".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    };

    assert!(skill.matches_file("mod.rs"));
    assert!(skill.matches_file("src/mod.rs"));
    assert!(skill.matches_file("src/tools/mod.rs"));
    assert!(!skill.matches_file("src/main.rs"));
    assert!(!skill.matches_file("module.rs"));

    Ok(())
}

#[test]
fn test_skill_matches_file_directory_prefix() -> Result<()> {
    let skill = Skill {
        name: "src-skill".to_string(),
        description: None,
        triggers: vec!["src/**".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    };

    assert!(skill.matches_file("src/main.rs"));
    assert!(skill.matches_file("src/lib.rs"));
    assert!(skill.matches_file("src/nested/deep/file.rs"));
    assert!(!skill.matches_file("tests/main.rs"));
    assert!(!skill.matches_file("main.rs"));

    Ok(())
}

#[test]
fn test_skill_activate_deactivate() -> Result<()> {
    let mut manager = SkillManager::new();

    let skill = Skill {
        name: "test".to_string(),
        description: None,
        triggers: vec![],
        content: "Test content".to_string(),
        source_path: None,
        active: false,
    };

    manager.register(skill);

    // Initially not active
    assert!(manager.get_active().is_empty());

    // Activate
    assert!(manager.activate("test"));
    assert_eq!(manager.get_active().len(), 1);

    // Deactivate
    assert!(manager.deactivate("test"));
    assert!(manager.get_active().is_empty());

    // Try to activate non-existent skill
    assert!(!manager.activate("nonexistent"));

    Ok(())
}

#[test]
fn test_skill_get_matching_files() -> Result<()> {
    let mut manager = SkillManager::new();

    manager.register(Skill {
        name: "rust".to_string(),
        description: None,
        triggers: vec!["*.rs".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    });

    manager.register(Skill {
        name: "python".to_string(),
        description: None,
        triggers: vec!["*.py".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    });

    let rust_matches = manager.get_matching("src/main.rs");
    assert_eq!(rust_matches.len(), 1);
    assert_eq!(rust_matches[0].name, "rust");

    let python_matches = manager.get_matching("script.py");
    assert_eq!(python_matches.len(), 1);
    assert_eq!(python_matches[0].name, "python");

    let no_matches = manager.get_matching("file.txt");
    assert!(no_matches.is_empty());

    Ok(())
}

#[test]
fn test_skill_auto_activate_for_files() -> Result<()> {
    let mut manager = SkillManager::new();

    manager.register(Skill {
        name: "rust".to_string(),
        description: None,
        triggers: vec!["*.rs".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    });

    manager.register(Skill {
        name: "python".to_string(),
        description: None,
        triggers: vec!["*.py".to_string()],
        content: String::new(),
        source_path: None,
        active: false,
    });

    // Auto-activate based on files
    manager.auto_activate_for_files(&["src/main.rs", "lib.rs"]);

    let active = manager.get_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].name, "rust");

    Ok(())
}

#[test]
fn test_skill_to_prompt_injection() -> Result<()> {
    let skill = Skill {
        name: "my-skill".to_string(),
        description: Some("A helpful skill".to_string()),
        triggers: vec![],
        content: "# Best Practices\n\n1. Do this\n2. Do that".to_string(),
        source_path: None,
        active: true,
    };

    let prompt = skill.to_prompt_injection();

    assert!(prompt.contains("# Skill: my-skill"));
    assert!(prompt.contains("*A helpful skill*"));
    assert!(prompt.contains("# Best Practices"));

    Ok(())
}

#[test]
fn test_skill_get_active_skills_prompt() -> Result<()> {
    let mut manager = SkillManager::new();

    manager.register(Skill {
        name: "skill1".to_string(),
        description: Some("First skill".to_string()),
        triggers: vec![],
        content: "Content 1".to_string(),
        source_path: None,
        active: true,
    });

    manager.register(Skill {
        name: "skill2".to_string(),
        description: None,
        triggers: vec![],
        content: "Content 2".to_string(),
        source_path: None,
        active: false, // Not active
    });

    manager.activate("skill1");

    let prompt = manager.get_active_skills_prompt();
    assert!(prompt.is_some());

    let prompt_content = prompt.unwrap();
    assert!(prompt_content.contains("# Active Skills"));
    assert!(prompt_content.contains("skill1"));
    assert!(prompt_content.contains("Content 1"));

    Ok(())
}

#[test]
fn test_skill_get_active_skills_prompt_none() -> Result<()> {
    let manager = SkillManager::new();

    // No active skills
    let prompt = manager.get_active_skills_prompt();
    assert!(prompt.is_none());

    Ok(())
}

#[test]
fn test_builtin_rust_skill() -> Result<()> {
    let skill = builtin::rust_skill();

    assert_eq!(skill.name, "rust-patterns");
    assert!(skill.description.is_some());
    assert!(skill.triggers.contains(&"*.rs".to_string()));
    assert!(skill.content.contains("Error Handling"));
    assert!(skill.content.contains("Ownership"));

    Ok(())
}

#[test]
fn test_builtin_react_skill() -> Result<()> {
    let skill = builtin::react_skill();

    assert_eq!(skill.name, "react-patterns");
    assert!(skill.description.is_some());
    assert!(skill.triggers.contains(&"*.tsx".to_string()));
    assert!(skill.triggers.contains(&"*.jsx".to_string()));
    assert!(skill.content.contains("React"));
    assert!(skill.content.contains("hooks"));

    Ok(())
}

#[test]
fn test_builtin_python_skill() -> Result<()> {
    let skill = builtin::python_skill();

    assert_eq!(skill.name, "python-patterns");
    assert!(skill.description.is_some());
    assert!(skill.triggers.contains(&"*.py".to_string()));
    assert!(skill.content.contains("Type Hints"));
    assert!(skill.content.contains("Docstrings"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_skill_load_from_file() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create a skill file
    temp_dir.child("my-skill.md").write_str(r#"---
name: custom-skill
trigger: "*.custom"
description: A custom skill
---

# Custom Skill

Custom instructions here.
"#)?;

    let manager = SkillManager::new();
    let skill = manager.load_skill_file(temp_dir.child("my-skill.md").path()).await?;

    assert_eq!(skill.name, "custom-skill");
    assert_eq!(skill.triggers, vec!["*.custom".to_string()]);
    assert!(skill.content.contains("Custom Skill"));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_skill_load_from_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create multiple skill files
    temp_dir.child("skill1.md").write_str(r#"---
name: skill-one
---

# Skill One
"#)?;

    temp_dir.child("skill2.md").write_str(r#"---
name: skill-two
---

# Skill Two
"#)?;

    // Create a non-markdown file (should be ignored)
    temp_dir.child("readme.txt").write_str("Not a skill")?;

    let mut manager = SkillManager::new();
    let loaded = manager.load_from_directory(temp_dir.path()).await?;

    assert_eq!(loaded, 2);
    assert!(manager.get("skill-one").is_some());
    assert!(manager.get("skill-two").is_some());

    Ok(())
}

#[test]
fn test_skill_manager_add_search_path() -> Result<()> {
    let mut manager = SkillManager::new();

    let path1 = PathBuf::from("/path/to/skills1");
    let path2 = PathBuf::from("/path/to/skills2");

    manager.add_search_path(path1.clone());
    manager.add_search_path(path2.clone());

    // Adding same path again should not duplicate
    manager.add_search_path(path1.clone());

    // We can't directly access search_paths, but we can verify via behavior
    // The manager should have unique paths

    Ok(())
}

#[test]
fn test_skill_multiple_triggers_match() -> Result<()> {
    let skill = Skill {
        name: "web-skill".to_string(),
        description: None,
        triggers: vec![
            "*.html".to_string(),
            "*.css".to_string(),
            "*.js".to_string(),
        ],
        content: String::new(),
        source_path: None,
        active: false,
    };

    assert!(skill.matches_file("index.html"));
    assert!(skill.matches_file("styles.css"));
    assert!(skill.matches_file("app.js"));
    assert!(!skill.matches_file("main.rs"));

    Ok(())
}

#[test]
fn test_skill_get_returns_reference() -> Result<()> {
    let mut manager = SkillManager::new();

    manager.register(Skill {
        name: "test".to_string(),
        description: Some("Test description".to_string()),
        triggers: vec![],
        content: "Test content".to_string(),
        source_path: None,
        active: false,
    });

    let skill = manager.get("test");
    assert!(skill.is_some());

    let skill = skill.unwrap();
    assert_eq!(skill.name, "test");
    assert_eq!(skill.description, Some("Test description".to_string()));

    Ok(())
}

#[test]
fn test_skill_get_nonexistent() -> Result<()> {
    let manager = SkillManager::new();

    let skill = manager.get("nonexistent");
    assert!(skill.is_none());

    Ok(())
}
