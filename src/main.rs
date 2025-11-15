mod config;
mod git;
mod isolation;
mod llm;
mod session;
mod tools;
mod tui;
mod vm;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use session::Session;

#[derive(Parser)]
#[command(name = "safe-coder")]
#[command(about = "AI coding assistant with Firecracker VM isolation", long_about = None)]
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
        /// Run in demo mode (no VM required)
        #[arg(long, default_value = "false")]
        demo: bool,
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
        Commands::Config { show, api_key, model } => {
            handle_config(show, api_key, model)?;
        }
        Commands::Init { path } => {
            init_project(path)?;
        }
    }

    Ok(())
}

async fn run_chat(project_path: PathBuf, use_tui: bool, demo: bool) -> Result<()> {
    let canonical_path = project_path.canonicalize()?;

    // Demo mode - no VM or API required
    if demo && use_tui {
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.run_demo().await?;
        return Ok(());
    }

    let config = Config::load()?;
    let mut session = Session::new(config, canonical_path.clone()).await?;

    // Start isolation environment
    session.start(canonical_path.clone()).await?;

    if use_tui {
        // Use TUI mode
        let mut tui_runner = tui::TuiRunner::new(canonical_path.display().to_string());
        tui_runner.run(session).await?;
        return Ok(());
    }

    // Classic CLI mode
    println!("🔥 Safe Coder - AI Coding Assistant with Firecracker VM Isolation");
    println!("Project: {}", canonical_path.display());
    println!("Type 'exit' or 'quit' to end the session\n");

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

        if input == "exit" || input == "quit" {
            println!("\nStopping VM and cleaning up...");
            session.stop().await?;
            println!("Goodbye!");
            break;
        }

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

    Ok(())
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

fn init_project(path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path)?;

    println!("✓ Initialized safe-coder project at: {}", path.display());
    println!("\nNext steps:");
    println!("  1. Configure your API key: safe-coder config --api-key YOUR_API_KEY");
    println!("  2. Start coding: safe-coder chat --path {}", path.display());

    Ok(())
}
