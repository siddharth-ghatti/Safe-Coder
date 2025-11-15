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
use std::time::Duration;
use tokio::sync::mpsc;

mod app;
mod ui;
mod messages;
mod spinner;
mod banner;

pub use app::App;
pub use messages::{ChatMessage, MessageType, ToolExecution};

use crate::session::Session;

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
        self.app.start_vm("demo-vm-12345".to_string());
        self.app.add_system_message("Demo mode - no VM required. Type 'exit' to quit.");
        self.app.add_user_message("Create a hello.rs file");
        self.app.add_assistant_message("I'll create a simple Hello World program in Rust for you.");

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

    pub async fn run(&mut self, mut session: Session) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create channels for communication
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Run the app
        let result = self.run_app(&mut terminal, &mut session, &mut rx, &tx).await;

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
        session: &mut Session,
        rx: &mut mpsc::UnboundedReceiver<String>,
        tx: &mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| ui::draw(f, &mut self.app))?;

            // Handle events with a timeout
            if event::poll(Duration::from_millis(100))? {
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

                                // Add user message
                                self.app.add_user_message(&input);
                                self.app.set_thinking(true);

                                // Send to LLM
                                let tx_clone = tx.clone();
                                let input_clone = input.clone();
                                tokio::spawn(async move {
                                    let _ = tx_clone.send(input_clone);
                                });

                                // Process message
                                match session.send_message(input).await {
                                    Ok(response) => {
                                        self.app.set_thinking(false);
                                        if !response.is_empty() {
                                            self.app.add_assistant_message(&response);
                                        }
                                    }
                                    Err(e) => {
                                        self.app.set_thinking(false);
                                        self.app.add_error_message(&format!("Error: {}", e));
                                    }
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
                        _ => {}
                    }
                }
            }

            // Check for async messages
            while let Ok(msg) = rx.try_recv() {
                self.app.set_status(&format!("Processing: {}", msg));
            }

            self.app.tick();
        }

        Ok(())
    }

    async fn run_demo_app<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        loop {
            terminal.draw(|f| ui::draw(f, &mut self.app))?;

            // Handle events with a timeout
            if event::poll(Duration::from_millis(100))? {
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
                                    "This is demo mode. In production, I would process your request using the LLM and execute tools in the Firecracker VM."
                                );

                                // Add a demo tool execution
                                let mut tool = ToolExecution::new("demo_tool".to_string(), input.clone());
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
