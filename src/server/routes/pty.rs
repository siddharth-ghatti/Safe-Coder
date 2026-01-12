//! PTY WebSocket endpoint for terminal emulation

use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::server::state::AppState;

/// GET /api/sessions/:id/pty - WebSocket endpoint for PTY
pub async fn pty_websocket(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_pty_connection(socket, state, session_id))
}

async fn handle_pty_connection(socket: WebSocket, state: Arc<AppState>, session_id: String) {
    // Get session to find project path
    let project_path = {
        let sessions = state.sessions.read().await;
        match sessions.get(&session_id) {
            Some(handle) => handle.project_path.clone(),
            None => {
                tracing::error!("Session not found for PTY: {}", session_id);
                return;
            }
        }
    };

    // Create PTY
    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(pair) => pair,
        Err(e) => {
            tracing::error!("Failed to create PTY: {}", e);
            return;
        }
    };

    // Build command (bash or default shell)
    let mut cmd = CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()));
    cmd.cwd(&project_path);

    // Spawn shell
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(e) => {
            tracing::error!("Failed to spawn shell: {}", e);
            return;
        }
    };

    // Get master for reading/writing
    let master = pair.master;

    // Split WebSocket
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Clone master for read task
    let mut reader = match master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to clone PTY reader: {}", e);
            return;
        }
    };

    let mut writer = match master.take_writer() {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to get PTY writer: {}", e);
            return;
        }
    };

    // Spawn task to read from PTY and send to WebSocket
    let read_task = tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        loop {
            // Use blocking read in spawn_blocking for portable_pty
            let reader_clone = unsafe {
                // This is safe because we're the only owner
                std::mem::transmute::<&mut Box<dyn std::io::Read + Send>, &'static mut Box<dyn std::io::Read + Send>>(&mut reader)
            };

            let read_result = tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let mut local_buf = [0u8; 4096];
                match reader_clone.read(&mut local_buf) {
                    Ok(n) if n > 0 => Some(local_buf[..n].to_vec()),
                    _ => None,
                }
            })
            .await;

            match read_result {
                Ok(Some(data)) => {
                    if ws_sender.send(Message::Binary(data)).await.is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });

    // Spawn task to read from WebSocket and write to PTY
    let write_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Binary(data)) => {
                    // Use blocking write for portable_pty
                    let writer_clone = unsafe {
                        std::mem::transmute::<&mut Box<dyn std::io::Write + Send>, &'static mut Box<dyn std::io::Write + Send>>(&mut writer)
                    };
                    let data_clone = data.clone();

                    let _ = tokio::task::spawn_blocking(move || {
                        use std::io::Write;
                        let _ = writer_clone.write_all(&data_clone);
                        let _ = writer_clone.flush();
                    })
                    .await;
                }
                Ok(Message::Text(text)) => {
                    // Handle text messages (e.g., resize commands)
                    if text.starts_with("resize:") {
                        // Parse resize command: "resize:cols:rows"
                        let parts: Vec<&str> = text.split(':').collect();
                        if parts.len() == 3 {
                            if let (Ok(cols), Ok(rows)) = (parts[1].parse(), parts[2].parse()) {
                                let _ = master.resize(PtySize {
                                    rows,
                                    cols,
                                    pixel_width: 0,
                                    pixel_height: 0,
                                });
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to complete
    tokio::select! {
        _ = read_task => {}
        _ = write_task => {}
    }

    // Kill the child process
    let _ = child.kill();
}
