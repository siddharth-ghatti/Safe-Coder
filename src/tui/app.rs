use super::banner;
use super::messages::{BackgroundTask, BackgroundTaskStatus, ChatMessage, ToolExecution};
use super::sidebar::SidebarState;
use super::spinner::Spinner;
use super::theme_manager::ThemeManager;
use crate::tools::AgentMode;
use chrono::Local;
use std::path::PathBuf;

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
    pub session_status: SessionStatus,
    pub start_time: chrono::DateTime<Local>,
    /// Dirty flag - set to true when UI needs redraw
    pub needs_redraw: bool,
    /// Track last input length for cursor blink optimization
    last_input_len: usize,
    
    // Enhanced UI fields
    pub sidebar_state: SidebarState,
    pub current_session_id: Option<String>,
    pub show_help: bool,
    pub theme_manager: ThemeManager,
    /// Agent mode for tool availability (PLAN/BUILD)
    pub agent_mode: AgentMode,
}

#[derive(Debug, Clone)]
pub struct SessionStatus {
    pub active: bool,
    pub session_id: Option<String>,
    pub uptime: String,
    pub active_workspaces: usize,
}

impl App {
    pub fn new(project_path: String) -> Self {
        let mut theme_manager = ThemeManager::new(
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("safe-coder")
        );
        
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
            session_status: SessionStatus {
                active: false,
                session_id: None,
                uptime: "0s".to_string(),
                active_workspaces: 0,
            },
            start_time: Local::now(),
            needs_redraw: true,
            last_input_len: 0,
            
            // Enhanced UI fields
            sidebar_state: SidebarState::default(),
            current_session_id: None,
            show_help: false,
            theme_manager,
            agent_mode: AgentMode::default(),
        };

        // Add the banner as a welcome message
        let welcome = format!(
            "{}\n\nWelcome to Safe Coder!\n\nProject: {}\n\nI'm an AI coding assistant. I can:\n  - Read and write files in your project\n  - Edit files with precise string replacements\n  - Execute bash commands\n  - Help with debugging, code review, refactoring, and more\n\nWhat would you like to work on today?",
            banner::BANNER_SMALL.trim(),
            project_path
        );
        app.messages.push(ChatMessage::system(welcome));

        app
    }

    pub fn tick(&mut self) {
        let old_frame = self.animation_frame;

        if self.is_thinking {
            self.spinner.tick();
            // Always redraw when thinking (for spinner animation)
            self.needs_redraw = true;
        }

        // Increment animation frame
        self.animation_frame = (self.animation_frame + 1) % 100;

        // Check if cursor blink state changed (every 10 frames)
        let old_cursor_visible = (old_frame % 20) < 10;
        let new_cursor_visible = (self.animation_frame % 20) < 10;
        if old_cursor_visible != new_cursor_visible {
            self.needs_redraw = true;
        }

        // Update session uptime if session is active (only every ~1 second worth of ticks)
        if self.session_status.active && self.animation_frame % 10 == 0 {
            let elapsed = Local::now()
                .signed_duration_since(self.start_time)
                .num_seconds();
            let new_uptime = if elapsed < 60 {
                format!("{}s", elapsed)
            } else if elapsed < 3600 {
                format!("{}m {}s", elapsed / 60, elapsed % 60)
            } else {
                format!("{}h {}m", elapsed / 3600, (elapsed % 3600) / 60)
            };
            if new_uptime != self.session_status.uptime {
                self.session_status.uptime = new_uptime;
                self.needs_redraw = true;
            }
        }
    }

    /// Mark the app as needing a redraw
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }

    /// Clear the dirty flag after drawing
    pub fn clear_dirty(&mut self) {
        self.needs_redraw = false;
    }

    pub fn input_push(&mut self, c: char) {
        self.input.push(c);
        self.needs_redraw = true;
    }

    pub fn input_pop(&mut self) {
        self.input.pop();
        self.needs_redraw = true;
    }

    pub fn input_submit(&mut self) -> String {
        let input = self.input.clone();
        self.input.clear();
        self.scroll_to_bottom();
        self.needs_redraw = true;
        input
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::user(content.to_string()));
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages
            .push(ChatMessage::assistant(content.to_string()));
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    pub fn add_system_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::system(content.to_string()));
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    pub fn add_error_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::error(content.to_string()));
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    pub fn add_tool_message(&mut self, content: &str) {
        self.messages.push(ChatMessage::tool(content.to_string()));
        self.scroll_to_bottom();
        self.needs_redraw = true;
    }

    pub fn add_tool_execution(&mut self, tool: ToolExecution) {
        self.tool_executions.push(tool);
        self.needs_redraw = true;
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = status.to_string();
        self.needs_redraw = true;
    }

    pub fn set_thinking(&mut self, thinking: bool) {
        self.is_thinking = thinking;
        if !thinking {
            self.processing_message.clear();
        }
        self.needs_redraw = true;
    }

    pub fn set_processing_message(&mut self, message: &str) {
        self.processing_message = message.to_string();
        self.needs_redraw = true;
    }

    pub fn start_session(&mut self, session_id: String) {
        self.session_status.active = true;
        self.session_status.session_id = Some(session_id.clone());
        self.current_session_id = Some(session_id.clone());
        self.start_time = Local::now();
        self.add_system_message(&format!("Session started: {}", session_id));
    }

    pub fn stop_session(&mut self) {
        if let Some(session_id) = &self.session_status.session_id {
            self.add_system_message(&format!("Session ended: {}", session_id));
        }
        self.session_status.active = false;
        self.session_status.session_id = None;
        self.current_session_id = None;
    }

    pub fn update_workspace_count(&mut self, count: usize) {
        self.session_status.active_workspaces = count;
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
            self.needs_redraw = true;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
        self.needs_redraw = true;
    }

    pub fn scroll_page_up(&mut self) {
        if self.scroll_offset >= 10 {
            self.scroll_offset -= 10;
        } else {
            self.scroll_offset = 0;
        }
        self.needs_redraw = true;
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_offset += 10;
        self.needs_redraw = true;
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
        self.needs_redraw = true;
    }

    // Background task management methods
    pub fn add_background_task(&mut self, task: BackgroundTask) {
        self.background_tasks.push(task);
        self.is_orchestrating = true;
    }

    pub fn update_task_status(&mut self, task_id: &str, status: BackgroundTaskStatus) {
        if let Some(task) = self
            .background_tasks
            .iter_mut()
            .find(|t| t.task_id == task_id)
        {
            task.status = status;
        }
        // Check if any tasks are still running
        self.is_orchestrating = self.background_tasks.iter().any(|t| {
            matches!(
                t.status,
                BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending
            )
        });
    }

    pub fn complete_task(&mut self, task_id: &str, output: String) {
        if let Some(task) = self
            .background_tasks
            .iter_mut()
            .find(|t| t.task_id == task_id)
        {
            task.complete(output);
        }
        self.check_orchestration_complete();
    }

    pub fn fail_task(&mut self, task_id: &str, error: String) {
        if let Some(task) = self
            .background_tasks
            .iter_mut()
            .find(|t| t.task_id == task_id)
        {
            task.fail(error);
        }
        self.check_orchestration_complete();
    }

    fn check_orchestration_complete(&mut self) {
        self.is_orchestrating = self.background_tasks.iter().any(|t| {
            matches!(
                t.status,
                BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending
            )
        });
    }

    pub fn add_orchestration_message(&mut self, content: &str) {
        self.messages
            .push(ChatMessage::orchestration(content.to_string()));
        self.scroll_to_bottom();
    }

    pub fn get_active_tasks_count(&self) -> usize {
        self.background_tasks
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    BackgroundTaskStatus::Running | BackgroundTaskStatus::Pending
                )
            })
            .count()
    }

    pub fn get_completed_tasks_count(&self) -> usize {
        self.background_tasks
            .iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Completed))
            .count()
    }

    pub fn get_failed_tasks_count(&self) -> usize {
        self.background_tasks
            .iter()
            .filter(|t| matches!(t.status, BackgroundTaskStatus::Failed(_)))
            .count()
    }

    // Enhanced UI methods
    pub fn is_focused_on_input(&self) -> bool {
        matches!(self.focus, FocusPanel::Chat)
    }

    pub fn visible_messages(&self) -> impl Iterator<Item = &ChatMessage> {
        let total = self.messages.len();
        if self.scroll_offset >= total {
            [].iter()
        } else {
            let start = total - (total - self.scroll_offset.min(total));
            self.messages[start..].iter()
        }
    }

    pub fn show_help(&mut self) {
        self.show_help = true;
        self.mark_dirty();
    }

    pub fn hide_help(&mut self) {
        self.show_help = false;
        self.mark_dirty();
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        self.mark_dirty();
    }

    pub async fn cycle_theme(&mut self) -> anyhow::Result<()> {
        let new_theme = self.theme_manager.cycle_theme().await?;
        self.set_status(&format!("Switched to {} theme", new_theme));
        Ok(())
    }

    pub async fn toggle_high_contrast(&mut self) -> anyhow::Result<()> {
        self.theme_manager.toggle_high_contrast().await?;
        let status = if self.theme_manager.is_high_contrast() {
            "High contrast enabled"
        } else {
            "High contrast disabled"
        };
        self.set_status(status);
        Ok(())
    }

    pub async fn load_theme_config(&mut self) -> anyhow::Result<()> {
        self.theme_manager.load().await?;
        Ok(())
    }
    
    pub fn cycle_theme_sync(&mut self) {
        self.theme_manager.cycle_theme_sync();
        let theme_name = self.theme_manager.current_theme_name();
        self.set_status(&format!("Switched to {} theme", theme_name));
    }

    pub fn cycle_agent_mode(&mut self) {
        self.agent_mode = self.agent_mode.next();
        self.set_status(&format!("Agent mode: {} - {}", self.agent_mode.short_name(), self.agent_mode.description()));
    }

    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
        self.mark_dirty();
    }
}
