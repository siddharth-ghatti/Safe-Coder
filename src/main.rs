mod approval;
mod auth;
mod checkpoint;
mod commands;
mod config;
mod custom_commands;
mod git;
mod llm;
mod memory;
mod orchestrator;
mod persistence;
mod session;
mod tools;
mod tui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use commands::{CommandParser, CommandResult};
use config::Config;
use orchestrator::{Orchestrator, OrchestratorConfig, WorkerKind};
use session::Session;

#[derive(Parser)]
#[command(name = "safe-coder")]
#[command(about = "AI coding orchestrator that delegates to Claude Code, Gemini CLI, and other AI agents", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start an interactive coding session
    Chat {
        /// Path to the project directory (default: current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
        /// Use the TUI (Terminal User Interface) mode
        #[arg(long, default_value = "true")]
        tui: bool,
        /// Run in demo mode (no LLM API required)
        #[arg(long, default_value = "false")]
        demo: bool,
    },
    /// Orchestrate a task by delegating to external AI CLIs
    Orchestrate {
        /// The task or request to execute
        #[arg(short, long)]
        task: Option<String>,
        /// Path to the project directory (default: current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
        /// Preferred worker: claude, gemini (default: claude)
        #[arg(short, long, default_value = "claude")]
        worker: String,
        /// Use git worktrees for isolation (default: true)
        #[arg(long, default_value = "true")]
        worktrees: bool,
        /// Maximum concurrent workers
        #[arg(long, default_value = "3")]
        max_workers: usize,
        /// Maximum concurrent Claude workers (overrides config)
        #[arg(long)]
        claude_max: Option<usize>,
        /// Maximum concurrent Gemini workers (overrides config)
        #[arg(long)]
        gemini_max: Option<usize>,
        /// Delay between starting workers in milliseconds (overrides config)
        #[arg(long)]
        start_delay_ms: Option<u64>,
    },
    /// Configure safe-coder
    Config {
        /// Show current configuration
        #[arg(short, long)]
        show: bool,
        /// Set API key
        #[arg(long)]
        api_key: Option<String>,
        /// Set model
        #[arg(long)]
        model: Option<String>,
    },
    /// Login to a provider using device flow authentication
    Login {
        /// Provider to login to (anthropic or github-copilot)
        provider: String,
    },
    /// Initialize a new project
    Init {
        /// Path to initialize (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "safe_coder=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Chat { path, tui, demo } => {
            run_chat(path, tui, demo).await?;
        }
        Commands::Orchestrate { task, path, worker, worktrees, max_workers, claude_max, gemini_max, start_delay_ms } => {
            run_orchestrate(task, path, worker, worktrees, max_workers, claude_max, gemini_max, start_delay_ms).await?;
        }
        Commands::Config { show, api_key, model } => {
            handle_config(show, api_key, model)?;
        }
        Commands::Login { provider } => {
            handle_login(&provider).await?;
        }
        Commands::Init { path } => {
            init_project(path)?;
        }
    }

    Ok(())
}

async fn run_chat(project_path: PathBuf, use_tui: bool, demo: bool) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;

    // Demo mode - no API required
    if demo && use_tui {
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.run_demo().await?;
        return Ok(());
    }

    let config = Config::load()?;
    let mut session = Session::new(config, canonical_path.clone()).await?;

    // Initialize session (git tracking, etc.)
    session.start().await?;

    if use_tui {
        // Use TUI mode
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.run(session).await?;
        return Ok(());
    }

    // Classic CLI mode
    println!("🤖 Safe Coder - AI Coding Assistant with Git Safety");
    println!("Project: {}", canonical_path.display());
    println!("Type '/help' for commands or 'exit' to quit\n");

    // Interactive loop
    loop {
        print!("\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // Parse command
        let parsed_cmd = CommandParser::parse(input);

        // Execute command
        match commands::execute_command(parsed_cmd, &mut session).await {
            Ok(CommandResult::Exit) => {
                println!("\nEnding session...");
                session.stop().await?;
                println!("✨ Session ended. All changes tracked in git. Goodbye!");
                break;
            },
            Ok(CommandResult::Clear) => {
                // Clear screen
                print!("\x1B[2J\x1B[1;1H");
                continue;
            },
            Ok(CommandResult::Message(msg)) => {
                println!("\n{}", msg);
                continue;
            },
            Ok(CommandResult::ModifiedInput(modified_input)) => {
                // Send the modified input to the AI
                match session.send_message(modified_input).await {
                    Ok(response) => {
                        if !response.is_empty() {
                            println!("\n{}", response);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            },
            Ok(CommandResult::Continue) => {
                // Continue normally - send to AI
                match session.send_message(input.to_string()).await {
                    Ok(response) => {
                        if !response.is_empty() {
                            println!("\n{}", response);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            },
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}

/// Run the orchestrator to delegate tasks to external CLI agents
async fn run_orchestrate(
    task: Option<String>,
    project_path: PathBuf,
    worker: String,
    use_worktrees: bool,
    max_workers: usize,
    claude_max: Option<usize>,
    gemini_max: Option<usize>,
    start_delay_ms: Option<u64>,
) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;
    
    // Parse worker preference
    let default_worker = match worker.to_lowercase().as_str() {
        "claude" | "claude-code" => WorkerKind::ClaudeCode,
        "gemini" | "gemini-cli" => WorkerKind::GeminiCli,
        _ => {
            eprintln!("Unknown worker '{}'. Using claude.", worker);
            WorkerKind::ClaudeCode
        }
    };
    
    // Load config for throttle limits
    let user_config = Config::load().unwrap_or_default();
    
    // Create orchestrator config (CLI args override config file)
    let config = orchestrator::OrchestratorConfig {
        claude_cli_path: Some(user_config.orchestrator.claude_cli_path.clone()),
        gemini_cli_path: Some(user_config.orchestrator.gemini_cli_path.clone()),
        max_workers,
        default_worker,
        use_worktrees,
        throttle_limits: orchestrator::ThrottleLimits {
            claude_max_concurrent: claude_max.unwrap_or(user_config.orchestrator.throttle_limits.claude_max_concurrent),
            gemini_max_concurrent: gemini_max.unwrap_or(user_config.orchestrator.throttle_limits.gemini_max_concurrent),
            start_delay_ms: start_delay_ms.unwrap_or(user_config.orchestrator.throttle_limits.start_delay_ms),
        },
    };
    
    // Create orchestrator
    let mut orchestrator = Orchestrator::new(canonical_path.clone(), config).await?;
    
    println!("🎯 Safe Coder Orchestrator");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Project: {}", canonical_path.display());
    println!("Default worker: {:?}", orchestrator.config.default_worker);
    println!("Max concurrent workers: {}", orchestrator.config.max_workers);
    println!("Using worktrees: {}", use_worktrees);
    println!("Throttle limits:");
    println!("  - Claude max concurrent: {}", orchestrator.config.throttle_limits.claude_max_concurrent);
    println!("  - Gemini max concurrent: {}", orchestrator.config.throttle_limits.gemini_max_concurrent);
    println!("  - Start delay: {}ms", orchestrator.config.throttle_limits.start_delay_ms);
    println!();
    
    // If task provided via CLI, execute it directly
    if let Some(task_text) = task {
        println!("📋 Processing task: {}", task_text);
        println!();
        
        match orchestrator.process_request(&task_text).await {
            Ok(response) => {
                println!("{}", response.summary);
            }
            Err(e) => {
                eprintln!("❌ Orchestration failed: {}", e);
            }
        }
        
        // Cleanup
        orchestrator.cleanup().await?;
        return Ok(());
    }
    
    // Interactive mode
    println!("Enter tasks to orchestrate (type 'exit' to quit, 'status' for worker status):");
    println!();
    
    loop {
        print!("🎯 > ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        match input.to_lowercase().as_str() {
            "exit" | "quit" => {
                println!("\n🧹 Cleaning up workspaces...");
                orchestrator.cleanup().await?;
                println!("✨ Orchestrator session ended. Goodbye!");
                break;
            }
            "status" => {
                let statuses = orchestrator.get_status().await;
                if statuses.is_empty() {
                    println!("No active workers.");
                } else {
                    println!("📊 Worker Status:");
                    for status in statuses {
                        println!("  - Task {}: {:?} ({:?})", 
                            status.task_id, 
                            status.state,
                            status.kind
                        );
                    }
                }
                continue;
            }
            "cancel" => {
                println!("🛑 Cancelling all workers...");
                orchestrator.cancel_all().await?;
                println!("All workers cancelled.");
                continue;
            }
            "help" => {
                print_orchestrator_help();
                continue;
            }
            _ => {}
        }
        
        // Process the request
        println!("\n📋 Planning task: {}", input);
        println!();
        
        match orchestrator.process_request(input).await {
            Ok(response) => {
                println!("\n{}", response.summary);
            }
            Err(e) => {
                eprintln!("❌ Error: {}", e);
            }
        }
    }
    
    Ok(())
}

fn print_orchestrator_help() {
    println!();
    println!("🎯 Orchestrator Commands:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  exit/quit  - End the session and cleanup");
    println!("  status     - Show status of all workers");
    println!("  cancel     - Cancel all running workers");
    println!("  help       - Show this help message");
    println!();
    println!("Enter any other text to orchestrate a task.");
    println!("The task will be broken down and delegated to AI agents.");
    println!();
}

fn handle_config(show: bool, api_key: Option<String>, model: Option<String>) -> Result<()> {
    let mut config = Config::load()?;

    if show {
        println!("Current configuration:");
        println!("{}", toml::to_string_pretty(&config)?);
        return Ok(());
    }

    let mut changed = false;

    if let Some(key) = api_key {
        config.llm.api_key = Some(key);
        changed = true;
        println!("API key updated");
    }

    if let Some(m) = model {
        config.llm.model = m;
        changed = true;
        println!("Model updated");
    }

    if changed {
        config.save()?;
        println!("Configuration saved to: {:?}", Config::config_path()?);
    } else {
        println!("No changes made. Use --show to view current configuration.");
    }

    Ok(())
}

async fn handle_login(provider: &str) -> Result<()> {
    use auth::{run_device_flow, DeviceFlowAuth};
    use config::{Config, LlmProvider};

    let llm_provider = match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => LlmProvider::Anthropic,
        "github-copilot" | "copilot" => LlmProvider::GitHubCopilot,
        _ => {
            anyhow::bail!(
                "Unknown provider '{}'. Supported: anthropic, github-copilot",
                provider
            );
        }
    };

    let token = match llm_provider {
        LlmProvider::GitHubCopilot => {
            let auth = auth::github_copilot::GitHubCopilotAuth::new();
            run_device_flow(&auth, "GitHub Copilot").await?
        }
        LlmProvider::Anthropic => {
            let auth = auth::anthropic::AnthropicAuth::new();
            run_device_flow(&auth, "Anthropic (Claude)").await?
        }
        _ => {
            anyhow::bail!("Provider does not support device flow authentication");
        }
    };

    // Save the token
    let token_path = Config::token_path(&llm_provider)?;
    token.save(&token_path)?;

    println!("✅ Token saved to: {:?}", token_path);
    println!("\nYou can now use safe-coder with your {} subscription!", provider);

    Ok(())
}

fn init_project(path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path)?;

    println!("✓ Initialized safe-coder project at: {}", path.display());
    println!("\nNext steps:");
    println!("  1. Configure your API key: safe-coder config --api-key YOUR_API_KEY");
    println!("  2. Or login with device flow: safe-coder login github-copilot");
    println!("  3. Start coding: safe-coder chat --path {}", path.display());

    Ok(())
}
