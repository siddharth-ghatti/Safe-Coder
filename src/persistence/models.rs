use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Saved chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub id: String,
    pub name: Option<String>,
    pub project_path: String,
    pub messages: String, // JSON serialized
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub total_messages: usize,
    pub total_tokens_sent: usize,
    pub total_tokens_received: usize,
    pub total_tool_calls: usize,
    pub session_duration_secs: i64,
    pub tools_used: Vec<ToolUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUsage {
    pub tool_name: String,
    pub count: usize,
}

impl SessionStats {
    pub fn new() -> Self {
        Self {
            total_messages: 0,
            total_tokens_sent: 0,
            total_tokens_received: 0,
            total_tool_calls: 0,
            session_duration_secs: 0,
            tools_used: Vec::new(),
        }
    }

    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("📊 Session Statistics\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        // Duration
        let duration = self.session_duration_secs;
        let hours = duration / 3600;
        let minutes = (duration % 3600) / 60;
        let seconds = duration % 60;

        output.push_str(&format!("⏱️  Duration: {}h {}m {}s\n", hours, minutes, seconds));
        output.push_str(&format!("💬 Messages: {}\n", self.total_messages));
        output.push_str(&format!("🔧 Tool Calls: {}\n\n", self.total_tool_calls));

        // Tokens
        output.push_str("📝 Token Usage:\n");
        output.push_str(&format!("   Sent:     {:>8}\n", self.total_tokens_sent));
        output.push_str(&format!("   Received: {:>8}\n", self.total_tokens_received));
        output.push_str(&format!("   Total:    {:>8}\n\n", self.total_tokens_sent + self.total_tokens_received));

        // Tools used
        if !self.tools_used.is_empty() {
            output.push_str("🔨 Tools Used:\n");
            for tool in &self.tools_used {
                output.push_str(&format!("   {:<15} {}\n", tool.tool_name, tool.count));
            }
        }

        output
    }
}

impl Default for SessionStats {
    fn default() -> Self {
        Self::new()
    }
}
