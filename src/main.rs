mod approval;
mod auth;
mod cache;
mod checkpoint;
mod commands;
mod config;
mod context;
mod custom_commands;
mod git;
mod llm;
mod loop_detector;
mod lsp;
mod mcp;
mod memory;
mod orchestrator;
mod permissions;
mod persistence;
mod planning;
mod prompts;
mod server;
mod session;
mod shell;
mod subagent;
mod tools;
mod tui;
mod unified_planning;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use commands::{CommandParser, CommandResult};
use config::Config;
use orchestrator::{Orchestrator, WorkerKind};
use session::Session;

#[derive(Parser)]
#[command(name = "safe-coder")]
#[command(about = "AI coding shell with interactive shell interface and AI assistance", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to the project directory (default: current directory)
    #[arg(short, long, default_value = ".", global = true)]
    path: PathBuf,

    /// Automatically connect to AI on startup (only for shell mode)
    #[arg(long, global = true)]
    ai: bool,

    /// Use legacy text-based shell instead of TUI (only for shell mode)
    #[arg(long, global = true)]
    no_tui: bool,

    /// Resume a previous session (shows interactive picker if no ID provided)
    #[arg(long, global = true)]
    resume: bool,

    /// Resume the most recent session
    #[arg(long, global = true)]
    resume_last: bool,

    /// Resume a specific session by ID
    #[arg(long, global = true, value_name = "SESSION_ID")]
    resume_id: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the interactive shell with AI assistance (default mode)
    ///
    /// This is the primary way to use safe-coder. Run shell commands directly
    /// in a visual interface with optional AI assistance. Features include:
    ///   - Full shell functionality with command history
    ///   - Real-time command output streaming
    ///   - AI assistance with context awareness
    ///   - Git integration and safety features
    ///   - TUI or text-based interface options
    ///
    /// AI Commands (when connected):
    ///   ai-connect        - Connect to AI assistant
    ///   ai <query>        - Ask AI for help with shell context
    ///   chat              - Enter interactive coding mode
    #[command(alias = "sh")]
    Shell {
        /// Path to the project directory (default: current directory)
        #[arg(short, long, default_value = ".")]
        path: PathBuf,
        /// Automatically connect to AI on startup
        #[arg(long)]
        ai: bool,
        /// Use legacy text-based shell (no TUI)
        #[arg(long)]
        no_tui: bool,
    },
    /// Legacy interactive coding session (chat-first mode)
    ///
    /// This is the legacy chat-focused interface. Consider using the shell
    /// mode instead for a more integrated experience.
    #[command(alias = "c")]
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
        /// Execution mode: plan (deep planning with approval) or act (auto-execute)
        #[arg(short, long, default_value = "act")]
        mode: String,
    },
    /// Orchestrate complex tasks by delegating to multiple AI agents
    #[command(alias = "orch")]
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
        /// Execution mode: plan (create plan and wait for approval) or act (auto-execute)
        #[arg(short, long, default_value = "act")]
        mode: String,
    },
    /// Configure safe-coder settings and authentication
    #[command(alias = "cfg")]
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
    /// Logout and clear stored credentials
    Logout {
        /// Provider to logout from (anthropic, github-copilot, or all)
        #[arg(default_value = "all")]
        provider: String,
    },
    /// Initialize a new project with safe-coder
    Init {
        /// Path to initialize (default: current directory)
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Resume a previous session
    Resume {
        /// Session ID to resume (shows picker if not provided)
        session_id: Option<String>,
        /// Resume the most recent session instead of showing picker
        #[arg(long)]
        last: bool,
    },
    /// Start HTTP server for desktop app integration
    ///
    /// This starts an HTTP/WebSocket server that exposes safe-coder's
    /// functionality via REST APIs and real-time event streams.
    /// The server can be used by the desktop app or other clients.
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "9876")]
        port: u16,
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Enable CORS for cross-origin requests (for development)
        #[arg(long)]
        cors: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Only initialize tracing for non-TUI modes
    // TUI mode uses its own rendering and tracing would interfere with the alternate screen
    let use_tui = match &cli.command {
        Some(Commands::Chat { tui: true, .. }) => true,
        Some(Commands::Shell { no_tui: false, .. }) => true,
        None if !cli.no_tui => true, // Default shell mode uses TUI
        _ => false,
    };

    if !use_tui {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "safe_coder=info,tower_http=info".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    }

    match cli.command.unwrap_or(Commands::Shell {
        path: cli.path,
        ai: cli.ai,
        no_tui: cli.no_tui,
    }) {
        Commands::Shell { path, ai, no_tui } => {
            if no_tui {
                // Legacy text-based shell
                run_shell_legacy(path, ai).await?;
            } else {
                // New shell-first TUI (Warp-like)
                run_shell_tui(path, ai).await?;
            }
        }
        Commands::Chat {
            path,
            tui,
            demo,
            mode,
        } => {
            run_chat(path, tui, demo, mode).await?;
        }
        Commands::Orchestrate {
            task,
            path,
            worker,
            worktrees,
            max_workers,
            claude_max,
            gemini_max,
            start_delay_ms,
            mode,
        } => {
            run_orchestrate(
                task,
                path,
                worker,
                worktrees,
                max_workers,
                claude_max,
                gemini_max,
                start_delay_ms,
                mode,
            )
            .await?;
        }
        Commands::Config {
            show,
            api_key,
            model,
        } => {
            handle_config(show, api_key, model)?;
        }
        Commands::Login { provider } => {
            handle_login(&provider).await?;
        }
        Commands::Logout { provider } => {
            handle_logout(&provider)?;
        }
        Commands::Init { path } => {
            init_project(path)?;
        }
        Commands::Resume { session_id, last } => {
            handle_resume(session_id, last).await?;
        }
        Commands::Serve { port, host, cors } => {
            run_server(port, host, cors).await?;
        }
    }

    Ok(())
}

/// Run the HTTP server for desktop app integration
async fn run_server(port: u16, host: String, cors: bool) -> Result<()> {
    // Tracing is already initialized in main() for non-TUI modes
    let config = server::ServerConfig {
        port,
        host,
        cors_enabled: cors,
    };

    server::start_server(config).await
}

async fn run_chat(project_path: PathBuf, use_tui: bool, demo: bool, mode: String) -> Result<()> {
    use approval::UserMode;

    let canonical_path = project_path.canonicalize()?;

    // Parse user mode
    let user_mode = UserMode::from_str(&mode)?;

    // Demo mode - no API required
    if demo && use_tui {
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.initialize().await?;
        tui_runner.run_demo().await?;
        return Ok(());
    }

    let config = Config::load()?;
    let mut session = Session::new(config, canonical_path.clone()).await?;

    // Set user mode
    session.set_user_mode(user_mode);

    // Show mode on startup
    let mode_desc = match user_mode {
        UserMode::Plan => "PLAN mode - deep planning with approval before execution",
        UserMode::Build => "BUILD mode - lightweight planning with auto-execution",
    };

    if use_tui {
        // Use TUI mode - skip session.start() as it outputs to stdout and interferes with TUI
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.initialize().await?;
        tui_runner.run(session).await?;
        return Ok(());
    }

    // Initialize session (git tracking, etc.) - only for non-TUI mode
    session.start().await?;

    // Classic CLI mode
    println!("🤖 Safe Coder - AI Coding Assistant with Git Safety");
    println!("Project: {}", canonical_path.display());
    println!("Mode: {}", mode_desc);
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
            }
            Ok(CommandResult::Clear) => {
                // Clear screen
                print!("\x1B[2J\x1B[1;1H");
                continue;
            }
            Ok(CommandResult::Message(msg)) => {
                println!("\n{}", msg);
                continue;
            }
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
            }
            Ok(CommandResult::ShowCommandsModal) => {
                // In CLI mode, just print the commands (no modal)
                use crate::commands::slash::get_commands_text;
                println!("\n{}", get_commands_text());
                continue;
            }
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
            }
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
    mode: String,
) -> Result<()> {
    use approval::UserMode;

    let canonical_path = project_path.canonicalize()?;

    // Parse user mode
    let user_mode = UserMode::from_str(&mode)?;

    // Load config for throttle limits
    let user_config = Config::load().unwrap_or_default();

    // Helper function to parse worker string to WorkerKind
    fn parse_worker_kind(s: &str) -> WorkerKind {
        match s.to_lowercase().as_str() {
            "claude" | "claude-code" => WorkerKind::ClaudeCode,
            "gemini" | "gemini-cli" => WorkerKind::GeminiCli,
            "safe-coder" | "safecoder" => WorkerKind::SafeCoder,
            "github-copilot" | "copilot" | "gh-copilot" => WorkerKind::GitHubCopilot,
            _ => WorkerKind::ClaudeCode,
        }
    }

    // Parse worker preference
    let default_worker = parse_worker_kind(&worker);

    // Parse worker strategy from config
    let worker_strategy = match user_config
        .orchestrator
        .worker_strategy
        .to_lowercase()
        .as_str()
    {
        "single" | "single-worker" => orchestrator::WorkerStrategy::SingleWorker,
        "round-robin" | "roundrobin" => orchestrator::WorkerStrategy::RoundRobin,
        "task-based" | "taskbased" => orchestrator::WorkerStrategy::TaskBased,
        "load-balanced" | "loadbalanced" => orchestrator::WorkerStrategy::LoadBalanced,
        _ => orchestrator::WorkerStrategy::SingleWorker,
    };

    // Parse enabled workers from config
    let enabled_workers: Vec<WorkerKind> = user_config
        .orchestrator
        .enabled_workers
        .iter()
        .map(|s| parse_worker_kind(s))
        .collect();

    // Create orchestrator config (CLI args override config file)
    let config = orchestrator::OrchestratorConfig {
        claude_cli_path: Some(user_config.orchestrator.claude_cli_path.clone()),
        gemini_cli_path: Some(user_config.orchestrator.gemini_cli_path.clone()),
        safe_coder_cli_path: Some(user_config.orchestrator.safe_coder_cli_path.clone()),
        gh_cli_path: Some(user_config.orchestrator.gh_cli_path.clone()),
        max_workers,
        default_worker,
        worker_strategy,
        enabled_workers,
        use_worktrees,
        throttle_limits: orchestrator::ThrottleLimits {
            claude_max_concurrent: claude_max.unwrap_or(
                user_config
                    .orchestrator
                    .throttle_limits
                    .claude_max_concurrent,
            ),
            gemini_max_concurrent: gemini_max.unwrap_or(
                user_config
                    .orchestrator
                    .throttle_limits
                    .gemini_max_concurrent,
            ),
            safe_coder_max_concurrent: user_config
                .orchestrator
                .throttle_limits
                .safe_coder_max_concurrent,
            copilot_max_concurrent: user_config
                .orchestrator
                .throttle_limits
                .copilot_max_concurrent,
            start_delay_ms: start_delay_ms
                .unwrap_or(user_config.orchestrator.throttle_limits.start_delay_ms),
        },
        user_mode,
    };

    // Create orchestrator
    let mut orchestrator = Orchestrator::new(canonical_path.clone(), config).await?;

    let mode_desc = match user_mode {
        UserMode::Plan => "PLAN (requires approval before execution)",
        UserMode::Build => "BUILD (auto-execute)",
    };

    println!("🎯 Safe Coder Orchestrator");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Project: {}", canonical_path.display());
    println!("Mode: {}", mode_desc);
    println!("Default worker: {:?}", orchestrator.config.default_worker);
    println!(
        "Max concurrent workers: {}",
        orchestrator.config.max_workers
    );
    println!("Using worktrees: {}", use_worktrees);
    println!("Throttle limits:");
    println!(
        "  - Claude max concurrent: {}",
        orchestrator.config.throttle_limits.claude_max_concurrent
    );
    println!(
        "  - Gemini max concurrent: {}",
        orchestrator.config.throttle_limits.gemini_max_concurrent
    );
    println!(
        "  - Start delay: {}ms",
        orchestrator.config.throttle_limits.start_delay_ms
    );
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
                        println!(
                            "  - Task {}: {:?} ({:?})",
                            status.task_id, status.state, status.kind
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
    use auth::run_device_flow;
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
            // Use the new OAuth PKCE flow for Anthropic
            handle_anthropic_login().await?
        }
        _ => {
            anyhow::bail!("Provider does not support authentication");
        }
    };

    // Save the token
    let token_path = Config::token_path(&llm_provider)?;
    token.save(&token_path)?;

    println!("\nToken saved to: {:?}", token_path);
    println!(
        "\nYou can now use safe-coder with your {} account!",
        provider
    );

    Ok(())
}

fn handle_logout(provider: &str) -> Result<()> {
    use config::{Config, LlmProvider};

    let providers_to_clear: Vec<LlmProvider> = match provider.to_lowercase().as_str() {
        "anthropic" | "claude" => vec![LlmProvider::Anthropic],
        "github-copilot" | "copilot" => vec![LlmProvider::GitHubCopilot],
        "all" => vec![LlmProvider::Anthropic, LlmProvider::GitHubCopilot],
        _ => {
            anyhow::bail!(
                "Unknown provider '{}'. Supported: anthropic, github-copilot, all",
                provider
            );
        }
    };

    let mut cleared_any = false;

    for llm_provider in providers_to_clear {
        if let Ok(token_path) = Config::token_path(&llm_provider) {
            if token_path.exists() {
                match std::fs::remove_file(&token_path) {
                    Ok(_) => {
                        println!("Cleared credentials for {:?}", llm_provider);
                        cleared_any = true;
                    }
                    Err(e) => {
                        eprintln!("Failed to remove {:?}: {}", token_path, e);
                    }
                }
            }
        }
    }

    if cleared_any {
        println!("\nCredentials cleared. Run 'safe-coder login <provider>' to re-authenticate.");
    } else {
        println!("No stored credentials found to clear.");
    }

    Ok(())
}

async fn handle_anthropic_login() -> Result<auth::StoredToken> {
    use auth::anthropic::{AnthropicAuth, AuthMode};

    println!("\nStarting Anthropic/Claude authentication...\n");

    // Ask which mode to use
    println!("Select login method:");
    println!("  1. Claude Pro/Max (use your Claude subscription)");
    println!("  2. Create API Key (generates an API key via console)");
    println!("  3. Enter API Key manually");
    print!("\nChoice [1/2/3]: ");
    io::stdout().flush()?;

    let mut choice = String::new();
    io::stdin().read_line(&mut choice)?;
    let choice = choice.trim();

    match choice {
        "1" => {
            let auth = AnthropicAuth::new();
            let pending = auth.start_authorization(AuthMode::ClaudeMax);

            println!("\nVisit this URL to authorize:");
            println!("{}\n", pending.url);
            println!("After authorizing, you'll get a code. Paste it below.");
            print!("\nAuthorization code: ");
            io::stdout().flush()?;

            let mut code = String::new();
            io::stdin().read_line(&mut code)?;
            let code = code.trim();

            if code.is_empty() {
                anyhow::bail!("Authorization code cannot be empty");
            }

            println!("\nExchanging code for token...");
            let token = auth
                .exchange_code(code, &pending.verifier, AuthMode::ClaudeMax)
                .await?;
            println!("Successfully authenticated with Claude Pro/Max!");

            Ok(token)
        }
        "2" => {
            let auth = AnthropicAuth::new();
            let pending = auth.start_authorization(AuthMode::Console);

            println!("\nVisit this URL to authorize:");
            println!("{}\n", pending.url);
            println!("After authorizing, you'll get a code. Paste it below.");
            print!("\nAuthorization code: ");
            io::stdout().flush()?;

            let mut code = String::new();
            io::stdin().read_line(&mut code)?;
            let code = code.trim();

            if code.is_empty() {
                anyhow::bail!("Authorization code cannot be empty");
            }

            println!("\nExchanging code for API key...");
            let token = auth
                .exchange_code(code, &pending.verifier, AuthMode::Console)
                .await?;
            println!("Successfully created API key!");

            Ok(token)
        }
        "3" => {
            print!("\nEnter your API key: ");
            io::stdout().flush()?;

            let mut api_key = String::new();
            io::stdin().read_line(&mut api_key)?;
            let api_key = api_key.trim().to_string();

            if api_key.is_empty() {
                anyhow::bail!("API key cannot be empty");
            }

            println!("API key saved!");

            Ok(auth::StoredToken::Api { key: api_key })
        }
        _ => {
            anyhow::bail!("Invalid choice. Please enter 1, 2, or 3.");
        }
    }
}

fn init_project(path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path)?;

    println!("✓ Initialized safe-coder project at: {}", path.display());
    println!("\nNext steps:");
    println!("  1. Configure authentication:");
    println!("     safe-coder login anthropic        # Login with Claude");
    println!("     safe-coder login github-copilot   # Login with GitHub Copilot");
    println!("     # OR manually set API key:");
    println!("     safe-coder config --api-key YOUR_API_KEY");
    println!();
    println!("  2. Start the interactive shell:");
    println!("     cd {}", path.display());
    println!("     safe-coder                        # Starts shell mode");
    println!("     safe-coder --ai                   # Starts with AI connected");
    println!();
    println!("  3. In the shell, use AI commands:");
    println!("     ai-connect                        # Connect to AI");
    println!("     ai how do I list files?           # Ask for help");
    println!("     chat                              # Enter interactive coding mode");

    Ok(())
}

/// Run the new shell-first TUI mode (Warp-like)
async fn run_shell_tui(project_path: PathBuf, connect_ai: bool) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;
    tui::run_shell_tui(canonical_path, connect_ai).await
}

/// Run the legacy text-based shell mode
async fn run_shell_legacy(project_path: PathBuf, connect_ai: bool) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;
    shell::run_shell(canonical_path, connect_ai).await
}

/// Handle session resumption
async fn handle_resume(session_id: Option<String>, last: bool) -> Result<()> {
    use persistence::event_log::EventLogger;

    // Get the session to resume
    let session_info = if last {
        // Get the most recent session
        match EventLogger::get_last_session()? {
            Some(info) => info,
            None => {
                println!("No previous sessions found.");
                println!("\nRun 'safe-coder' to start a new session.");
                return Ok(());
            }
        }
    } else if let Some(id) = session_id {
        // Load specific session by ID
        let sessions = EventLogger::list_recent_sessions(30)?;
        match sessions.into_iter().find(|s| s.session_id == id) {
            Some(info) => info,
            None => {
                println!("Session not found: {}", id);
                println!("\nUse 'safe-coder resume' to see available sessions.");
                return Ok(());
            }
        }
    } else {
        // Show interactive picker
        let sessions = EventLogger::list_recent_sessions(14)?;

        if sessions.is_empty() {
            println!("No previous sessions found.");
            println!("\nRun 'safe-coder' to start a new session.");
            return Ok(());
        }

        println!("📋 Recent Sessions");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

        for (i, session) in sessions.iter().enumerate() {
            println!(
                "  [{:2}] {} | {} | {} events",
                i + 1,
                session.session_id,
                session.created_at.format("%Y-%m-%d %H:%M"),
                session.event_count
            );
            println!("       Project: {}", session.project_path);
        }

        println!("\nEnter session number to resume (or 'q' to quit): ");

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "q" || input.is_empty() {
            return Ok(());
        }

        let idx: usize = input.parse().context("Invalid session number")?;
        if idx == 0 || idx > sessions.len() {
            anyhow::bail!("Invalid session number: {}", idx);
        }

        sessions.into_iter().nth(idx - 1).unwrap()
    };

    println!("\n🔄 Resuming session: {}", session_info.session_id);
    println!("   Project: {}", session_info.project_path);
    println!("   Created: {}", session_info.created_at.format("%Y-%m-%d %H:%M"));

    // Load messages from the session
    let messages = EventLogger::load_messages(&session_info.session_id)?;
    println!("   Messages restored: {}\n", messages.len());

    // Start TUI with the resumed session
    let project_path = PathBuf::from(&session_info.project_path);
    if project_path.exists() {
        let canonical_path = project_path.canonicalize()?;

        // Create session with restored messages
        let config = Config::load()?;
        let mut session = Session::new(config, canonical_path.clone()).await?;

        // Restore messages
        session.restore_messages(messages);

        // Run TUI
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.initialize().await?;
        tui_runner.run(session).await?;
    } else {
        println!("⚠️  Project path no longer exists: {}", session_info.project_path);
        println!("   Session messages can still be viewed in: {:?}", session_info.log_path);
    }

    Ok(())
}
