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
    Orchestration,
}

/// Represents a background worker task in the TUI
#[derive(Debug, Clone)]
pub struct BackgroundTask {
    pub task_id: String,
    pub description: String,
    pub worker_kind: String,
    pub status: BackgroundTaskStatus,
    pub output: Option<String>,
    pub started_at: DateTime<Local>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundTaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

impl BackgroundTask {
    pub fn new(task_id: String, description: String, worker_kind: String) -> Self {
        Self {
            task_id,
            description,
            worker_kind,
            status: BackgroundTaskStatus::Pending,
            output: None,
            started_at: Local::now(),
        }
    }

    pub fn set_running(&mut self) {
        self.status = BackgroundTaskStatus::Running;
    }

    pub fn complete(&mut self, output: String) {
        self.status = BackgroundTaskStatus::Completed;
        self.output = Some(output);
    }

    pub fn fail(&mut self, error: String) {
        self.status = BackgroundTaskStatus::Failed(error.clone());
        self.output = Some(error);
    }
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

    pub fn orchestration(content: String) -> Self {
        Self {
            message_type: MessageType::Orchestration,
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
