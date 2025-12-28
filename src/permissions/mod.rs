//! Permission Pattern Matching
//!
//! Allows users to approve patterns of tool usage rather than individual requests.
//! For example: "always approve read_file for src/**/*.rs"

use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Permission decision for a tool call
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Permission {
    /// Tool call is allowed
    Allowed,
    /// Tool call needs user approval
    NeedsApproval,
    /// Tool call is explicitly denied
    Denied,
}

/// A pattern that matches tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovedPattern {
    /// Tool name to match (exact match)
    pub tool_name: String,
    /// Parameter patterns (key -> glob pattern for value)
    pub param_patterns: HashMap<String, String>,
    /// Optional description for user reference
    pub description: Option<String>,
    /// Whether this is a permanent pattern (persisted) or session-only
    pub permanent: bool,
}

impl ApprovedPattern {
    /// Create a new pattern for a tool
    pub fn new(tool_name: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            param_patterns: HashMap::new(),
            description: None,
            permanent: false,
        }
    }

    /// Add a parameter pattern
    pub fn with_param(mut self, key: impl Into<String>, pattern: impl Into<String>) -> Self {
        self.param_patterns.insert(key.into(), pattern.into());
        self
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Make the pattern permanent
    pub fn permanent(mut self) -> Self {
        self.permanent = true;
        self
    }

    /// Check if this pattern matches a tool call
    pub fn matches(&self, tool_name: &str, params: &Value) -> bool {
        // Tool name must match exactly
        if self.tool_name != tool_name {
            return false;
        }

        // If no param patterns, match any params for this tool
        if self.param_patterns.is_empty() {
            return true;
        }

        // All param patterns must match
        for (key, pattern) in &self.param_patterns {
            let param_value = params.get(key).and_then(|v| v.as_str());

            match param_value {
                Some(value) => {
                    // Try to match the glob pattern
                    match Pattern::new(pattern) {
                        Ok(glob) => {
                            if !glob.matches(value) {
                                return false;
                            }
                        }
                        Err(_) => {
                            // Invalid pattern, fall back to exact match
                            if value != pattern {
                                return false;
                            }
                        }
                    }
                }
                None => {
                    // Parameter not found, doesn't match
                    return false;
                }
            }
        }

        true
    }
}

/// Manages permission patterns for tool execution
#[derive(Debug, Default)]
pub struct PermissionManager {
    /// Approved patterns (tool calls that match these are auto-approved)
    approved_patterns: Vec<ApprovedPattern>,
    /// Denied patterns (tool calls that match these are auto-denied)
    denied_patterns: Vec<ApprovedPattern>,
    /// Whether to use YOLO mode (approve everything)
    yolo_mode: bool,
}

impl PermissionManager {
    /// Create a new permission manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable YOLO mode (auto-approve everything)
    pub fn set_yolo_mode(&mut self, enabled: bool) {
        self.yolo_mode = enabled;
    }

    /// Check if YOLO mode is enabled
    pub fn is_yolo_mode(&self) -> bool {
        self.yolo_mode
    }

    /// Check permission for a tool call
    pub fn check(&self, tool_name: &str, params: &Value) -> Permission {
        // YOLO mode approves everything
        if self.yolo_mode {
            return Permission::Allowed;
        }

        // Check denied patterns first
        for pattern in &self.denied_patterns {
            if pattern.matches(tool_name, params) {
                return Permission::Denied;
            }
        }

        // Check approved patterns
        for pattern in &self.approved_patterns {
            if pattern.matches(tool_name, params) {
                return Permission::Allowed;
            }
        }

        // Default: needs approval
        Permission::NeedsApproval
    }

    /// Add an approved pattern
    pub fn approve_pattern(&mut self, pattern: ApprovedPattern) {
        self.approved_patterns.push(pattern);
    }

    /// Add a denied pattern
    pub fn deny_pattern(&mut self, pattern: ApprovedPattern) {
        self.denied_patterns.push(pattern);
    }

    /// Quick approval for a specific tool (no param restrictions)
    pub fn approve_tool(&mut self, tool_name: &str) {
        self.approved_patterns.push(ApprovedPattern::new(tool_name));
    }

    /// Quick approval for read operations in a directory
    pub fn approve_reads_in(&mut self, directory_pattern: &str) {
        self.approved_patterns.push(
            ApprovedPattern::new("read_file")
                .with_param("path", directory_pattern)
                .with_description(format!("Auto-approve reads in {}", directory_pattern)),
        );
    }

    /// Quick approval for edits in a directory
    pub fn approve_edits_in(&mut self, directory_pattern: &str) {
        self.approved_patterns.push(
            ApprovedPattern::new("edit_file")
                .with_param("file_path", directory_pattern)
                .with_description(format!("Auto-approve edits in {}", directory_pattern)),
        );
        self.approved_patterns.push(
            ApprovedPattern::new("write_file")
                .with_param("path", directory_pattern)
                .with_description(format!("Auto-approve writes in {}", directory_pattern)),
        );
    }

    /// Remove all session patterns (keep permanent ones)
    pub fn clear_session_patterns(&mut self) {
        self.approved_patterns.retain(|p| p.permanent);
        self.denied_patterns.retain(|p| p.permanent);
    }

    /// Get all approved patterns
    pub fn get_approved_patterns(&self) -> &[ApprovedPattern] {
        &self.approved_patterns
    }

    /// Get a summary of current permissions
    pub fn summary(&self) -> String {
        let mut output = String::new();

        if self.yolo_mode {
            output.push_str("Mode: YOLO (all tools auto-approved)\n");
            return output;
        }

        output.push_str("Mode: Pattern matching\n\n");

        if self.approved_patterns.is_empty() {
            output.push_str("Approved patterns: none\n");
        } else {
            output.push_str("Approved patterns:\n");
            for (i, pattern) in self.approved_patterns.iter().enumerate() {
                let desc = pattern.description.as_deref().unwrap_or(&pattern.tool_name);
                let permanent = if pattern.permanent {
                    " [permanent]"
                } else {
                    ""
                };
                output.push_str(&format!("  {}. {}{}\n", i + 1, desc, permanent));
            }
        }

        if !self.denied_patterns.is_empty() {
            output.push_str("\nDenied patterns:\n");
            for (i, pattern) in self.denied_patterns.iter().enumerate() {
                let desc = pattern.description.as_deref().unwrap_or(&pattern.tool_name);
                output.push_str(&format!("  {}. {}\n", i + 1, desc));
            }
        }

        output
    }

    /// Create common presets
    pub fn apply_preset(&mut self, preset: &str) {
        match preset {
            "safe" => {
                // Only read operations auto-approved
                self.approve_tool("read_file");
                self.approve_tool("list_file");
                self.approve_tool("glob");
                self.approve_tool("grep");
            }
            "dev" => {
                // Safe preset + edits to source files
                self.apply_preset("safe");
                self.approve_edits_in("src/**/*");
                self.approve_edits_in("tests/**/*");
            }
            "full" => {
                // Everything except dangerous bash commands
                self.approve_tool("read_file");
                self.approve_tool("list_file");
                self.approve_tool("glob");
                self.approve_tool("grep");
                self.approve_tool("write_file");
                self.approve_tool("edit_file");
                // Bash still needs approval
            }
            "yolo" => {
                self.yolo_mode = true;
            }
            _ => {
                tracing::warn!("Unknown permission preset: {}", preset);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pattern_exact_match() {
        let pattern = ApprovedPattern::new("read_file");

        assert!(pattern.matches("read_file", &json!({"path": "test.rs"})));
        assert!(!pattern.matches("write_file", &json!({"path": "test.rs"})));
    }

    #[test]
    fn test_pattern_with_param() {
        let pattern = ApprovedPattern::new("read_file").with_param("path", "src/**/*.rs");

        assert!(pattern.matches("read_file", &json!({"path": "src/main.rs"})));
        assert!(pattern.matches("read_file", &json!({"path": "src/lib/utils.rs"})));
        assert!(!pattern.matches("read_file", &json!({"path": "tests/test.rs"})));
        assert!(!pattern.matches("read_file", &json!({"path": "src/main.py"})));
    }

    #[test]
    fn test_permission_manager_yolo() {
        let mut manager = PermissionManager::new();
        manager.set_yolo_mode(true);

        assert_eq!(
            manager.check("bash", &json!({"command": "rm -rf /"})),
            Permission::Allowed
        );
    }

    #[test]
    fn test_permission_manager_patterns() {
        let mut manager = PermissionManager::new();
        manager.approve_tool("read_file");

        assert_eq!(
            manager.check("read_file", &json!({"path": "anything.txt"})),
            Permission::Allowed
        );
        assert_eq!(
            manager.check("write_file", &json!({"path": "anything.txt"})),
            Permission::NeedsApproval
        );
    }

    #[test]
    fn test_permission_manager_deny_overrides() {
        let mut manager = PermissionManager::new();
        manager.approve_tool("bash");
        manager.deny_pattern(ApprovedPattern::new("bash").with_param("command", "rm*"));

        // Regular bash allowed
        assert_eq!(
            manager.check("bash", &json!({"command": "ls -la"})),
            Permission::Allowed
        );

        // rm commands denied
        assert_eq!(
            manager.check("bash", &json!({"command": "rm -rf /tmp"})),
            Permission::Denied
        );
    }

    #[test]
    fn test_approve_reads_in() {
        let mut manager = PermissionManager::new();
        manager.approve_reads_in("src/**/*");

        assert_eq!(
            manager.check("read_file", &json!({"path": "src/main.rs"})),
            Permission::Allowed
        );
        assert_eq!(
            manager.check("read_file", &json!({"path": "tests/test.rs"})),
            Permission::NeedsApproval
        );
    }

    #[test]
    fn test_preset_safe() {
        let mut manager = PermissionManager::new();
        manager.apply_preset("safe");

        assert_eq!(
            manager.check("read_file", &json!({"path": "any.txt"})),
            Permission::Allowed
        );
        assert_eq!(
            manager.check("glob", &json!({"pattern": "**/*.rs"})),
            Permission::Allowed
        );
        assert_eq!(
            manager.check("write_file", &json!({"path": "any.txt"})),
            Permission::NeedsApproval
        );
    }
}
