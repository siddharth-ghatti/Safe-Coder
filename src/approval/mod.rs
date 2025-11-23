use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Approval mode for tool execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalMode {
    /// Show execution plan before running
    Plan,
    /// Ask before each tool use (default)
    Default,
    /// Auto-approve file edits, ask for bash and other tools
    AutoEdit,
    /// Auto-approve everything (dangerous!)
    Yolo,
}

impl ApprovalMode {
    /// Parse approval mode from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "plan" => Ok(ApprovalMode::Plan),
            "default" => Ok(ApprovalMode::Default),
            "auto-edit" => Ok(ApprovalMode::AutoEdit),
            "yolo" => Ok(ApprovalMode::Yolo),
            _ => Err(anyhow::anyhow!(
                "Invalid approval mode. Valid modes: plan, default, auto-edit, yolo"
            )),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            ApprovalMode::Plan => "plan",
            ApprovalMode::Default => "default",
            ApprovalMode::AutoEdit => "auto-edit",
            ApprovalMode::Yolo => "yolo",
        }
    }

    /// Check if tool execution needs approval
    pub fn needs_approval(&self, tool_name: &str) -> bool {
        match self {
            ApprovalMode::Yolo => false,
            ApprovalMode::AutoEdit => {
                // Auto-approve read, write, edit; ask for bash and others
                !matches!(tool_name, "read_file" | "write_file" | "edit_file")
            },
            ApprovalMode::Default => true,
            ApprovalMode::Plan => true,
        }
    }

    /// Check if we should show execution plan
    pub fn should_show_plan(&self) -> bool {
        matches!(self, ApprovalMode::Plan)
    }
}

impl Default for ApprovalMode {
    fn default() -> Self {
        ApprovalMode::Default
    }
}

impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tool execution plan
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    pub tools: Vec<PlannedTool>,
}

#[derive(Debug, Clone)]
pub struct PlannedTool {
    pub name: String,
    pub description: String,
    pub requires_approval: bool,
}

impl ExecutionPlan {
    /// Create new execution plan
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    /// Add tool to plan
    pub fn add_tool(&mut self, name: String, description: String, requires_approval: bool) {
        self.tools.push(PlannedTool {
            name,
            description,
            requires_approval,
        });
    }

    /// Format plan for display
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("📋 Execution Plan\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        for (i, tool) in self.tools.iter().enumerate() {
            let approval_icon = if tool.requires_approval { "🔒" } else { "✅" };
            output.push_str(&format!(
                "{}. {} {} - {}\n",
                i + 1,
                approval_icon,
                tool.name,
                tool.description
            ));
        }

        output.push_str("\n");
        output.push_str("🔒 = Requires approval\n");
        output.push_str("✅ = Auto-approved\n");

        output
    }
}

impl Default for ExecutionPlan {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_mode_parse() {
        assert_eq!(ApprovalMode::from_str("plan").unwrap(), ApprovalMode::Plan);
        assert_eq!(ApprovalMode::from_str("default").unwrap(), ApprovalMode::Default);
        assert_eq!(ApprovalMode::from_str("auto-edit").unwrap(), ApprovalMode::AutoEdit);
        assert_eq!(ApprovalMode::from_str("yolo").unwrap(), ApprovalMode::Yolo);
    }

    #[test]
    fn test_needs_approval() {
        let mode = ApprovalMode::AutoEdit;
        assert!(!mode.needs_approval("write_file"));
        assert!(!mode.needs_approval("edit_file"));
        assert!(mode.needs_approval("bash"));

        let yolo = ApprovalMode::Yolo;
        assert!(!yolo.needs_approval("bash"));
    }
}
