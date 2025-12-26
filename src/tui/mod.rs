use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

mod app;
mod banner;
mod messages;
mod spinner;
mod ui;

pub use app::App;
pub use messages::{BackgroundTask, BackgroundTaskStatus, ChatMessage, MessageType, ToolExecution};

use crate::orchestrator::{Orchestrator, OrchestratorConfig};
use crate::session::Session;

/// Message from the LLM processing task
#[derive(Debug)]
enum LlmUpdate {
    Response(String),
    ToolCall { name: String, description: String },
    Error(String),
}

pub struct TuiRunner {
    app: App,
}

impl TuiRunner {
    pub fn new(project_path: String) -> Self {
        Self {
            app: App::new(project_path),
        }
    }

    pub async fn run_demo(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Add demo messages
        self.app.start_session("demo-session-12345".to_string());
        self.app
            .add_system_message("Demo mode - Type 'exit' to quit.");
        self.app.add_user_message("Create a hello.rs file");
        self.app
            .add_assistant_message("I'll create a simple Hello World program in Rust for you.");

        // Add some tool executions
        let mut tool1 = ToolExecution::new("write_file".to_string(), "hello.rs".to_string());
        tool1.complete("File created successfully".to_string());
        self.app.add_tool_execution(tool1);

        // Run the demo app
        let result = self.run_demo_app(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    pub async fn run(&mut self, session: Session) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Wrap session in Arc<Mutex> for shared access
        let session = Arc::new(Mutex::new(session));

        // Run the app
        let result = self.run_app(&mut terminal, session).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    async fn run_app<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        session: Arc<Mutex<Session>>,
    ) -> Result<()> {
        // Create orchestrator for background task handling
        let project_path = PathBuf::from(&self.app.project_path);
        let orchestrator_config = OrchestratorConfig::default();

        // Channel for orchestration updates
        let (orch_tx, mut orch_rx) = mpsc::unbounded_channel::<OrchestrationUpdate>();

        // Channel for LLM updates (async responses)
        let (llm_tx, mut llm_rx) = mpsc::unbounded_channel::<LlmUpdate>();

        loop {
            // Only redraw when needed
            if self.app.needs_redraw {
                terminal.draw(|f| ui::draw(f, &mut self.app))?;
                self.app.clear_dirty();
            }

            // Handle events with a shorter timeout for responsiveness
            if event::poll(Duration::from_millis(30))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.app.set_status("Shutting down...");
                            break;
                        }
                        KeyCode::Char(c) => {
                            self.app.input_push(c);
                        }
                        KeyCode::Backspace => {
                            self.app.input_pop();
                        }
                        KeyCode::Enter => {
                            let input = self.app.input_submit();
                            if !input.is_empty() {
                                if input == "exit" || input == "quit" {
                                    self.app.set_status("Shutting down...");
                                    break;
                                }

                                // Check if this is an orchestrate command
                                if input.starts_with("/orchestrate ") || input.starts_with("/orch ")
                                {
                                    let task_text = input
                                        .strip_prefix("/orchestrate ")
                                        .or_else(|| input.strip_prefix("/orch "))
                                        .unwrap_or("")
                                        .trim();

                                    if !task_text.is_empty() {
                                        self.app.add_user_message(&input);
                                        self.app.add_orchestration_message(&format!(
                                            "Orchestrating: {}",
                                            task_text
                                        ));
                                        self.app.set_status("Spawning workers...");

                                        // Spawn orchestration in background
                                        let project_path_clone = project_path.clone();
                                        let config_clone = orchestrator_config.clone();
                                        let task_text_owned = task_text.to_string();
                                        let orch_tx_clone = orch_tx.clone();

                                        tokio::spawn(async move {
                                            run_orchestration_background(
                                                project_path_clone,
                                                config_clone,
                                                task_text_owned,
                                                orch_tx_clone,
                                            )
                                            .await;
                                        });
                                    } else {
                                        self.app.add_error_message(
                                            "Usage: /orchestrate <task description>",
                                        );
                                    }
                                } else {
                                    // Regular message - send to LLM asynchronously
                                    self.app.add_user_message(&input);
                                    self.app.set_thinking(true);

                                    // Spawn LLM processing in background so UI stays responsive
                                    let session_clone = Arc::clone(&session);
                                    let llm_tx_clone = llm_tx.clone();
                                    let input_owned = input.clone();

                                    tokio::spawn(async move {
                                        let mut session = session_clone.lock().await;
                                        match session.send_message(input_owned).await {
                                            Ok(response) => {
                                                let _ = llm_tx_clone
                                                    .send(LlmUpdate::Response(response));
                                            }
                                            Err(e) => {
                                                let _ = llm_tx_clone
                                                    .send(LlmUpdate::Error(e.to_string()));
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Up => {
                            self.app.scroll_up();
                        }
                        KeyCode::Down => {
                            self.app.scroll_down();
                        }
                        KeyCode::PageUp => {
                            self.app.scroll_page_up();
                        }
                        KeyCode::PageDown => {
                            self.app.scroll_page_down();
                        }
                        KeyCode::Tab => {
                            self.app.cycle_focus();
                        }
                        KeyCode::Esc => {
                            // Could be used to cancel thinking in future
                        }
                        _ => {}
                    }
                }
            }

            // Check for LLM responses
            while let Ok(update) = llm_rx.try_recv() {
                match update {
                    LlmUpdate::Response(response) => {
                        self.app.set_thinking(false);
                        if !response.is_empty() {
                            self.app.add_assistant_message(&response);
                        }
                    }
                    LlmUpdate::ToolCall { name, description } => {
                        // Add tool call to chat as a distinct message
                        self.app
                            .add_tool_message(&format!("{}  {}", name, description));
                    }
                    LlmUpdate::Error(error) => {
                        self.app.set_thinking(false);
                        self.app.add_error_message(&format!("Error: {}", error));
                    }
                }
            }

            // Check for orchestration updates
            while let Ok(update) = orch_rx.try_recv() {
                match update {
                    OrchestrationUpdate::TaskStarted {
                        task_id,
                        description,
                        worker_kind,
                    } => {
                        let task =
                            BackgroundTask::new(task_id.clone(), description.clone(), worker_kind);
                        self.app.add_background_task(task);
                        self.app.add_orchestration_message(&format!(
                            "Task started: {} ({})",
                            description, task_id
                        ));
                        self.app.set_status(&format!(
                            "Running {} tasks...",
                            self.app.get_active_tasks_count()
                        ));
                    }
                    OrchestrationUpdate::TaskRunning { task_id } => {
                        self.app
                            .update_task_status(&task_id, BackgroundTaskStatus::Running);
                    }
                    OrchestrationUpdate::TaskCompleted { task_id, output } => {
                        self.app.complete_task(&task_id, output.clone());
                        self.app
                            .add_orchestration_message(&format!("Task completed: {}", task_id));
                        if self.app.get_active_tasks_count() == 0 {
                            self.app.set_status("All tasks completed");
                        }
                    }
                    OrchestrationUpdate::TaskFailed { task_id, error } => {
                        self.app.fail_task(&task_id, error.clone());
                        self.app
                            .add_error_message(&format!("Task failed ({}): {}", task_id, error));
                    }
                    OrchestrationUpdate::PlanCreated {
                        summary,
                        task_count,
                    } => {
                        self.app.add_orchestration_message(&format!(
                            "Plan created: {} tasks\n{}",
                            task_count, summary
                        ));
                    }
                    OrchestrationUpdate::AllComplete { summary } => {
                        self.app.add_orchestration_message(&format!(
                            "Orchestration complete!\n{}",
                            summary
                        ));
                        self.app.set_status("Ready");
                    }
                    OrchestrationUpdate::Error { message } => {
                        self.app
                            .add_error_message(&format!("Orchestration error: {}", message));
                        self.app.set_status("Ready");
                    }
                }
            }

            self.app.tick();
        }

        Ok(())
    }

    async fn run_demo_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Only redraw when needed
            if self.app.needs_redraw {
                terminal.draw(|f| ui::draw(f, &mut self.app))?;
                self.app.clear_dirty();
            }

            // Handle events with a shorter timeout for responsiveness
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            break;
                        }
                        KeyCode::Char(c) => {
                            self.app.input_push(c);
                        }
                        KeyCode::Backspace => {
                            self.app.input_pop();
                        }
                        KeyCode::Enter => {
                            let input = self.app.input_submit();
                            if !input.is_empty() {
                                if input == "exit" || input == "quit" {
                                    break;
                                }

                                // Add user message
                                self.app.add_user_message(&input);

                                // Simulate thinking with dynamic messages
                                self.app.set_thinking(true);

                                self.app.set_processing_message("Analyzing request");
                                tokio::time::sleep(Duration::from_millis(600)).await;

                                self.app.set_processing_message("Generating response");
                                tokio::time::sleep(Duration::from_millis(600)).await;

                                self.app.set_processing_message("Executing tools");
                                tokio::time::sleep(Duration::from_millis(600)).await;

                                self.app.set_thinking(false);

                                // Add demo response
                                self.app.add_assistant_message(
                                    "This is demo mode. In production, I would process your request using the LLM and execute tools in isolated git workspaces."
                                );

                                // Add a demo tool execution
                                let mut tool =
                                    ToolExecution::new("demo_tool".to_string(), input.clone());
                                tool.complete("Demo execution completed".to_string());
                                self.app.add_tool_execution(tool);
                            }
                        }
                        KeyCode::Up => {
                            self.app.scroll_up();
                        }
                        KeyCode::Down => {
                            self.app.scroll_down();
                        }
                        KeyCode::PageUp => {
                            self.app.scroll_page_up();
                        }
                        KeyCode::PageDown => {
                            self.app.scroll_page_down();
                        }
                        KeyCode::Tab => {
                            self.app.cycle_focus();
                        }
                        _ => {}
                    }
                }
            }

            self.app.tick();
        }

        Ok(())
    }
}

/// Updates from background orchestration tasks
#[derive(Debug, Clone)]
enum OrchestrationUpdate {
    PlanCreated {
        summary: String,
        task_count: usize,
    },
    TaskStarted {
        task_id: String,
        description: String,
        worker_kind: String,
    },
    TaskRunning {
        task_id: String,
    },
    TaskCompleted {
        task_id: String,
        output: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    AllComplete {
        summary: String,
    },
    Error {
        message: String,
    },
}

/// Run orchestration in background and send updates via channel
async fn run_orchestration_background(
    project_path: PathBuf,
    config: OrchestratorConfig,
    task_text: String,
    tx: mpsc::UnboundedSender<OrchestrationUpdate>,
) {
    // Create orchestrator
    let orchestrator_result = Orchestrator::new(project_path, config).await;

    let mut orchestrator = match orchestrator_result {
        Ok(o) => o,
        Err(e) => {
            let _ = tx.send(OrchestrationUpdate::Error {
                message: format!("Failed to create orchestrator: {}", e),
            });
            return;
        }
    };

    // Process the request
    // NOTE: The orchestrator currently processes tasks synchronously, so we receive
    // all results at once. Future improvement: refactor orchestrator to emit progress
    // events during execution for real-time UI updates.
    match orchestrator.process_request(&task_text).await {
        Ok(response) => {
            // Send plan created update
            let _ = tx.send(OrchestrationUpdate::PlanCreated {
                summary: response.plan.summary.clone(),
                task_count: response.plan.tasks.len(),
            });

            // Send task updates for each result
            for (i, task) in response.plan.tasks.iter().enumerate() {
                let result = &response.task_results[i];
                let worker_kind = format!("{:?}", result.worker_kind);

                // Send started notification
                let _ = tx.send(OrchestrationUpdate::TaskStarted {
                    task_id: task.id.clone(),
                    description: task.description.clone(),
                    worker_kind,
                });

                // Mark as running (even though it completed - for UI consistency)
                let _ = tx.send(OrchestrationUpdate::TaskRunning {
                    task_id: task.id.clone(),
                });

                // Small delay to allow UI to update
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;

                // Send result
                match &result.result {
                    Ok(output) => {
                        let _ = tx.send(OrchestrationUpdate::TaskCompleted {
                            task_id: task.id.clone(),
                            output: output.clone(),
                        });
                    }
                    Err(error) => {
                        let _ = tx.send(OrchestrationUpdate::TaskFailed {
                            task_id: task.id.clone(),
                            error: error.clone(),
                        });
                    }
                }
            }

            // Send completion
            let _ = tx.send(OrchestrationUpdate::AllComplete {
                summary: response.summary,
            });
        }
        Err(e) => {
            let _ = tx.send(OrchestrationUpdate::Error {
                message: e.to_string(),
            });
        }
    }

    // Cleanup
    let _ = orchestrator.cleanup().await;
}
