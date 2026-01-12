//! Tauri commands for the desktop app

use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Track the server port
static SERVER_PORT: AtomicU16 = AtomicU16::new(0);

/// Track the server process
static SERVER_PROCESS: Lazy<Mutex<Option<Child>>> = Lazy::new(|| Mutex::new(None));

/// Find the safe-coder binary
/// In production: uses the bundled binary
/// In development: falls back to system PATH or cargo target
fn find_safe_coder_binary(app: &AppHandle) -> Result<PathBuf, String> {
    // First, try the bundled binary (production mode)
    // Tauri resolves the sidecar binary with the correct platform suffix
    if let Ok(sidecar_path) = app.path().resource_dir() {
        let binary_name = if cfg!(windows) {
            "safe-coder.exe"
        } else {
            "safe-coder"
        };

        let sidecar = sidecar_path.join(binary_name);
        if sidecar.exists() {
            return Ok(sidecar);
        }
    }

    // Try using tauri's sidecar resolution
    #[cfg(not(debug_assertions))]
    {
        // In release mode, the binary should be bundled
        return Err(
            "Bundled safe-coder binary not found. Please rebuild the application.".to_string(),
        );
    }

    // Development mode: try system PATH
    #[cfg(debug_assertions)]
    {
        // Try the development server that should already be running
        // In dev mode, we expect the server to be started by npm run dev
        if let Ok(path) = which::which("safe-coder") {
            return Ok(path);
        }

        // Try cargo target directory
        let project_root = std::env::current_dir()
            .ok()
            .and_then(|p| p.parent()?.parent().map(|p| p.to_path_buf()));

        if let Some(root) = project_root {
            let dev_binary = root.join("target/release/safe-coder");
            if dev_binary.exists() {
                return Ok(dev_binary);
            }

            let debug_binary = root.join("target/debug/safe-coder");
            if debug_binary.exists() {
                return Ok(debug_binary);
            }
        }

        Err("safe-coder binary not found. Run 'cargo build --release' in the project root.".to_string())
    }
}

/// Start the safe-coder server
#[tauri::command]
pub async fn start_server(app: AppHandle, port: u16) -> Result<String, String> {
    // Check if server is already running
    {
        let process = SERVER_PROCESS.lock().map_err(|e| e.to_string())?;
        if process.is_some() {
            let current_port = SERVER_PORT.load(Ordering::SeqCst);
            return Ok(format!("Server already running on port {}", current_port));
        }
    }

    // In development mode, check if the server is already running (started by npm script)
    #[cfg(debug_assertions)]
    {
        if is_server_running(port).await {
            SERVER_PORT.store(port, Ordering::SeqCst);
            return Ok(format!("Connected to existing server on port {}", port));
        }
    }

    // Find the binary
    let binary_path = find_safe_coder_binary(&app)?;

    // Start the server
    let child = Command::new(&binary_path)
        .args(["serve", "--port", &port.to_string(), "--cors"])
        .spawn()
        .map_err(|e| format!("Failed to start server: {}", e))?;

    // Store the process and port
    {
        let mut process = SERVER_PROCESS.lock().map_err(|e| e.to_string())?;
        *process = Some(child);
    }
    SERVER_PORT.store(port, Ordering::SeqCst);

    // Wait for server to be ready
    for _ in 0..30 {
        if is_server_running(port).await {
            return Ok(format!("Server started on port {}", port));
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(format!("Server started on port {} (may still be initializing)", port))
}

/// Stop the safe-coder server
#[tauri::command]
pub fn stop_server() -> Result<(), String> {
    let mut process = SERVER_PROCESS.lock().map_err(|e| e.to_string())?;

    if let Some(mut child) = process.take() {
        // Try graceful shutdown first
        #[cfg(unix)]
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }

        // Give it a moment to shut down gracefully
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Force kill if still running
        let _ = child.kill();
        let _ = child.wait();
    }

    SERVER_PORT.store(0, Ordering::SeqCst);
    Ok(())
}

/// Check if server is responding
async fn is_server_running(port: u16) -> bool {
    // Simple TCP connection check
    tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .is_ok()
}

/// Get the current server URL
#[tauri::command]
pub fn get_server_url() -> String {
    let port = SERVER_PORT.load(Ordering::SeqCst);
    if port == 0 {
        "http://127.0.0.1:9876".to_string()
    } else {
        format!("http://127.0.0.1:{}", port)
    }
}

/// Check if the server is running
#[tauri::command]
pub async fn is_server_ready(port: u16) -> bool {
    is_server_running(port).await
}

/// Open a directory picker dialog
#[tauri::command]
pub async fn select_directory() -> Result<Option<String>, String> {
    // For now, return None - directory picker will be implemented with tauri dialog plugin
    // The frontend can use the native file dialog API
    Ok(None)
}

/// Open a path in the system file explorer
#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
