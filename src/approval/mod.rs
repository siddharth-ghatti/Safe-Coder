use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Execution mode for the agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Plan mode: Deep planning with user approval before execution
    /// Shows detailed analysis of what will be done and waits for user confirmation
    Plan,
    /// Act mode: Lighter planning that auto-executes
    /// Still plans internally but executes without asking
    Act,
}

impl ExecutionMode {
    /// Parse execution mode from string
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "plan" => Ok(ExecutionMode::Plan),
            "act" => Ok(ExecutionMode::Act),
            _ => Err(anyhow::anyhow!(
                "Invalid execution mode. Valid modes: plan, act"
            )),
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &str {
        match self {
            ExecutionMode::Plan => "plan",
            ExecutionMode::Act => "act",
        }
    }

    /// Check if this mode requires user approval before execution
    pub fn requires_approval(&self) -> bool {
        matches!(self, ExecutionMode::Plan)
    }

    /// Check if this mode should show detailed planning output
    pub fn show_detailed_plan(&self) -> bool {
        matches!(self, ExecutionMode::Plan)
    }
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Act
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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

/// Deep execution plan for Plan mode
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// Summary of the overall plan
    pub summary: String,
    /// Tools to be executed
    pub tools: Vec<PlannedTool>,
    /// Files that will be affected
    pub affected_files: Vec<String>,
    /// Potential risks identified
    pub risks: Vec<String>,
    /// Estimated complexity (1-5)
    pub complexity: u8,
}

#[derive(Debug, Clone)]
pub struct PlannedTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    pub expected_outcome: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn as_str(&self) -> &str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            RiskLevel::Low => "🟢",
            RiskLevel::Medium => "🟡",
            RiskLevel::High => "🔴",
        }
    }
}

impl ExecutionPlan {
    /// Create new execution plan
    pub fn new() -> Self {
        Self {
            summary: String::new(),
            tools: Vec::new(),
            affected_files: Vec::new(),
            risks: Vec::new(),
            complexity: 1,
        }
    }

    /// Create plan with a summary
    pub fn with_summary(summary: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            tools: Vec::new(),
            affected_files: Vec::new(),
            risks: Vec::new(),
            complexity: 1,
        }
    }

    /// Add tool to plan
    pub fn add_tool(&mut self, tool: PlannedTool) {
        // Track affected files from tool parameters
        if let Some(path) = tool.parameters.get("path").and_then(|v| v.as_str()) {
            if !self.affected_files.contains(&path.to_string()) {
                self.affected_files.push(path.to_string());
            }
        }
        if let Some(path) = tool.parameters.get("file_path").and_then(|v| v.as_str()) {
            if !self.affected_files.contains(&path.to_string()) {
                self.affected_files.push(path.to_string());
            }
        }
        self.tools.push(tool);
    }

    /// Add a simple tool (backwards compatible)
    pub fn add_simple_tool(&mut self, name: String, description: String, requires_approval: bool) {
        self.tools.push(PlannedTool {
            name,
            description: description.clone(),
            parameters: serde_json::json!({}),
            expected_outcome: description,
            risk_level: if requires_approval { RiskLevel::Medium } else { RiskLevel::Low },
        });
    }

    /// Add a risk to the plan
    pub fn add_risk(&mut self, risk: impl Into<String>) {
        self.risks.push(risk.into());
    }

    /// Set complexity level (1-5)
    pub fn set_complexity(&mut self, level: u8) {
        self.complexity = level.min(5).max(1);
    }

    /// Check if plan has any high-risk operations
    pub fn has_high_risk(&self) -> bool {
        self.tools.iter().any(|t| t.risk_level == RiskLevel::High) || !self.risks.is_empty()
    }

    /// Format plan for display (simple version)
    pub fn format(&self) -> String {
        self.format_detailed(false)
    }

    /// Format plan for display with optional deep analysis
    pub fn format_detailed(&self, deep: bool) -> String {
        let mut output = String::new();

        if deep {
            output.push_str("🎯 EXECUTION PLAN\n");
            output.push_str("══════════════════════════════════════════════════════════════\n\n");
        } else {
            output.push_str("📋 Execution Plan\n");
            output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");
        }

        // Summary
        if !self.summary.is_empty() {
            output.push_str(&format!("📝 Summary: {}\n\n", self.summary));
        }

        // Complexity indicator
        if deep {
            let complexity_bar = "█".repeat(self.complexity as usize) + &"░".repeat(5 - self.complexity as usize);
            output.push_str(&format!("📊 Complexity: [{}] ({}/5)\n\n", complexity_bar, self.complexity));
        }

        // Affected files
        if deep && !self.affected_files.is_empty() {
            output.push_str("📁 Files to be modified:\n");
            for file in &self.affected_files {
                output.push_str(&format!("   • {}\n", file));
            }
            output.push_str("\n");
        }

        // Tool execution steps
        output.push_str("🔧 Execution Steps:\n");
        for (i, tool) in self.tools.iter().enumerate() {
            let risk_icon = tool.risk_level.icon();
            output.push_str(&format!(
                "   {}. {} [{}] {}\n",
                i + 1,
                risk_icon,
                tool.name,
                tool.description
            ));

            if deep && !tool.expected_outcome.is_empty() && tool.expected_outcome != tool.description {
                output.push_str(&format!("      → Expected: {}\n", tool.expected_outcome));
            }
        }
        output.push_str("\n");

        // Risks
        if deep && !self.risks.is_empty() {
            output.push_str("⚠️  Potential Risks:\n");
            for risk in &self.risks {
                output.push_str(&format!("   • {}\n", risk));
            }
            output.push_str("\n");
        }

        // Legend
        output.push_str("Legend: 🟢 Low risk  🟡 Medium risk  🔴 High risk\n");

        output
    }
}

impl Default for ExecutionPlan {
    fn default() -> Self {
        Self::new()
    }
}

impl PlannedTool {
    /// Create a new planned tool
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::json!({}),
            expected_outcome: String::new(),
            risk_level: RiskLevel::Low,
        }
    }

    /// Set parameters
    pub fn with_params(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }

    /// Set expected outcome
    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.expected_outcome = outcome.into();
        self
    }

    /// Set risk level
    pub fn with_risk(mut self, level: RiskLevel) -> Self {
        self.risk_level = level;
        self
    }

    /// Determine risk level based on tool name and parameters
    pub fn auto_risk(mut self) -> Self {
        self.risk_level = match self.name.as_str() {
            "bash" => {
                // Check for dangerous bash commands
                if let Some(cmd) = self.parameters.get("command").and_then(|v| v.as_str()) {
                    if cmd.contains("rm ") || cmd.contains("sudo") || cmd.contains("chmod") {
                        RiskLevel::High
                    } else if cmd.contains("mv ") || cmd.contains("cp ") {
                        RiskLevel::Medium
                    } else {
                        RiskLevel::Low
                    }
                } else {
                    RiskLevel::Medium
                }
            }
            "write_file" => RiskLevel::Medium,
            "edit_file" => RiskLevel::Medium,
            "read_file" => RiskLevel::Low,
            _ => RiskLevel::Medium,
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_mode_parse() {
        assert_eq!(ExecutionMode::from_str("plan").unwrap(), ExecutionMode::Plan);
        assert_eq!(ExecutionMode::from_str("act").unwrap(), ExecutionMode::Act);
        assert!(ExecutionMode::from_str("invalid").is_err());
    }

    #[test]
    fn test_execution_mode_properties() {
        let plan = ExecutionMode::Plan;
        assert!(plan.requires_approval());
        assert!(plan.show_detailed_plan());

        let act = ExecutionMode::Act;
        assert!(!act.requires_approval());
        assert!(!act.show_detailed_plan());
    }

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

    #[test]
    fn test_execution_plan_format() {
        let mut plan = ExecutionPlan::with_summary("Test plan");
        plan.add_tool(
            PlannedTool::new("read_file", "Read config.rs")
                .with_params(serde_json::json!({"path": "src/config.rs"}))
                .auto_risk()
        );
        plan.add_tool(
            PlannedTool::new("bash", "Run tests")
                .with_params(serde_json::json!({"command": "cargo test"}))
                .auto_risk()
        );
        plan.set_complexity(2);

        let formatted = plan.format_detailed(true);
        assert!(formatted.contains("Test plan"));
        assert!(formatted.contains("read_file"));
        assert!(formatted.contains("bash"));
        assert!(formatted.contains("src/config.rs"));
    }

    #[test]
    fn test_planned_tool_auto_risk() {
        let read_tool = PlannedTool::new("read_file", "Read a file").auto_risk();
        assert_eq!(read_tool.risk_level, RiskLevel::Low);

        let write_tool = PlannedTool::new("write_file", "Write a file").auto_risk();
        assert_eq!(write_tool.risk_level, RiskLevel::Medium);

        let rm_tool = PlannedTool::new("bash", "Remove files")
            .with_params(serde_json::json!({"command": "rm -rf temp/"}))
            .auto_risk();
        assert_eq!(rm_tool.risk_level, RiskLevel::High);
    }
}
