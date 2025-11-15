use chrono::{DateTime, Local};

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType {
    User,
    Assistant,
    System,
    Error,
    Tool,
}

#[derive(Debug, Clone)]
pub struct ToolExecution {
    pub tool_name: String,
    pub parameters: String,
    pub result: Option<String>,
    pub status: ToolStatus,
    pub timestamp: DateTime<Local>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Success,
    Failed,
}

impl ChatMessage {
    pub fn user(content: String) -> Self {
        Self {
            message_type: MessageType::User,
            content,
            timestamp: Local::now(),
        }
    }

    pub fn assistant(content: String) -> Self {
        Self {
            message_type: MessageType::Assistant,
            content,
            timestamp: Local::now(),
        }
    }

    pub fn system(content: String) -> Self {
        Self {
            message_type: MessageType::System,
            content,
            timestamp: Local::now(),
        }
    }

    pub fn error(content: String) -> Self {
        Self {
            message_type: MessageType::Error,
            content,
            timestamp: Local::now(),
        }
    }

    pub fn tool(content: String) -> Self {
        Self {
            message_type: MessageType::Tool,
            content,
            timestamp: Local::now(),
        }
    }
}

impl ToolExecution {
    pub fn new(tool_name: String, parameters: String) -> Self {
        Self {
            tool_name,
            parameters,
            result: None,
            status: ToolStatus::Running,
            timestamp: Local::now(),
        }
    }

    pub fn complete(&mut self, result: String) {
        self.result = Some(result);
        self.status = ToolStatus::Success;
    }

    pub fn fail(&mut self, error: String) {
        self.result = Some(error);
        self.status = ToolStatus::Failed;
    }
}
