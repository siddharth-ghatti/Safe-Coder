use super::messages::{ChatMessage, ToolExecution, BackgroundTask, BackgroundTaskStatus};
use super::spinner::Spinner;
use chrono::Local;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusPanel {
    Chat,
    Tools,
    Status,
}

pub struct App {
    pub project_path: String,
    pub messages: Vec<ChatMessage>,
    pub tool_executions: Vec<ToolExecution>,
    pub background_tasks: Vec<BackgroundTask>,
    pub input: String,
    pub status: String,
    pub is_thinking: bool,
    pub is_orchestrating: bool,
    pub processing_message: String,
    pub animation_frame: usize,
    pub spinner: Spinner,
    pub scroll_offset: usize,
    pub focus: FocusPanel,
    pub vm_status: VmStatus,
    pub start_time: chrono::DateTime<Local>,
}

#[derive(Debug, Clone)]
pub struct VmStatus {
    pub running: bool,
    pub vm_id: Option<String>,
    pub uptime: String,
    pub memory_mb: usize,
    pub vcpus: u8,
}

impl App {
    pub fn new(project_path: String) -> Self {
        let mut app = Self {
            project_path: project_path.clone(),
            messages: Vec::new(),
            tool_executions: Vec::new(),
            background_tasks: Vec::new(),
            input: String::new(),
            status: "Ready".to_string(),
            is_thinking: false,
            is_orchestrating: false,
            processing_message: String::new(),
            animation_frame: 0,
            spinner: Spinner::new(),
            scroll_offset: 0,
            focus: FocusPanel::Chat,
            vm_status: VmStatus {
                running: false,
                vm_id: None,
                uptime: "0s".to_string(),
                memory_mb: 512,
                vcpus: 2,
            },
            start_time: Local::now(),
        };

        app.messages.push(ChatMessage::system(format!(
            "🔥 Safe Coder initialized\nProject: {}\nType your request or use /orchestrate <task> to delegate to AI agents...",
            project_path
        )));

        app
    }

    pub fn tick(&mut self) {
        if self.is_thinking {
            self.spinner.tick();
        }

        // Increment animation frame
        self.animation_frame = (self.animation_frame + 1) % 100;

        // Update VM uptime
        if self.vm_status.running {
            let elapsed = Local::now()
                .signed_duration_since(self.start_time)
                .num_seconds();
            self.vm_status.uptime = if elapsed < 60 {
                format!("{}s", elapsed)
            } else if elapsed < 3600 {
                format!("{}m {}s", elapsed / 60, elapsed % 60)
            } else {
                format!("{}h {}m", elapsed / 3600, (elapsed % 3600) / 60)
            };
        }
    }

    pub fn input_push(&mut self, c: char) {
        self.input.push(c);
    }

    pub fn input_pop(&mut self) {
        self.input.pop();
    }

    pub fn input_submit(&mut self) -> String {
        let input = self.input.clone();
        self.input.clear();
        self.scroll_to_bottom();
        input
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::user(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages
            .push(ChatMessage::assistant(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn add_system_message(&mut self, content: &str) {
        self.messages
            .push(ChatMessage::system(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn add_error_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::error(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn add_tool_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::tool(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn add_tool_execution(&mut self, tool: ToolExecution) {
        self.tool_executions.push(tool);
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = status.to_string();
    }

    pub fn set_thinking(&mut self, thinking: bool) {
        self.is_thinking = thinking;
        if !thinking {
            self.processing_message.clear();
        }
    }

    pub fn set_processing_message(&mut self, message: &str) {
        self.processing_message = message.to_string();
    }

    pub fn start_vm(&mut self, vm_id: String) {
        self.vm_status.running = true;
        self.vm_status.vm_id = Some(vm_id.clone());
        self.start_time = Local::now();
        self.add_system_message(&format!("VM started: {}", vm_id));
    }

    pub fn stop_vm(&mut self) {
        if let Some(vm_id) = &self.vm_status.vm_id {
            self.add_system_message(&format!("VM stopped: {}", vm_id));
        }
        self.vm_status.running = false;
        self.vm_status.vm_id = None;
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn scroll_page_up(&mut self) {
        if self.scroll_offset >= 10 {
            self.scroll_offset -= 10;
        } else {
            self.scroll_offset = 0;
        }
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_offset += 10;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPanel::Chat => FocusPanel::Tools,
            FocusPanel::Tools => FocusPanel::Status,
            FocusPanel::Status => FocusPanel::Chat,
        };
    }

    // Background task management methods
    pub fn add_background_task(&mut self, task: BackgroundTask) {
        self.background_tasks.push(task);
        self.is_orchestrating = true;
    }

    pub fn update_task_status(&mut self, task_id: &str, status: BackgroundTaskStatus) {
        if let Some(task) = self.background_tasks.iter_mut().find(|t| t.task_id == task_id) {
            task.status = status;
        }
        // Check if any tasks are still running
        self.is_orchestrating = self.background_tasks.iter()
            .any(|t| matches!(t.status, BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending));
    }

    pub fn complete_task(&mut self, task_id: &str, output: String) {
        if let Some(task) = self.background_tasks.iter_mut().find(|t| t.task_id == task_id) {
            task.complete(output);
        }
        self.check_orchestration_complete();
    }

    pub fn fail_task(&mut self, task_id: &str, error: String) {
        if let Some(task) = self.background_tasks.iter_mut().find(|t| t.task_id == task_id) {
            task.fail(error);
        }
        self.check_orchestration_complete();
    }

    fn check_orchestration_complete(&mut self) {
        self.is_orchestrating = self.background_tasks.iter()
            .any(|t| matches!(t.status, BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending));
    }

    pub fn add_orchestration_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::orchestration(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn get_active_tasks_count(&self) -> usize {
        self.background_tasks.iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending))
            .count()
    }

    pub fn get_completed_tasks_count(&self) -> usize {
        self.background_tasks.iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Completed))
            .count()
    }

    pub fn get_failed_tasks_count(&self) -> usize {
        self.background_tasks.iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Failed(_)))
            .count()
    }
}
