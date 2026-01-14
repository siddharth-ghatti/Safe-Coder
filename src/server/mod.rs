//! HTTP Server module for safe-coder desktop app integration
//!
//! This module provides an HTTP/WebSocket server that exposes safe-coder's
//! functionality via REST APIs and real-time event streams.

pub mod routes;
pub mod state;
pub mod types;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::config::Config;
use state::AppState;

/// Default port for the server
pub const DEFAULT_PORT: u16 = 9876;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub cors_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            host: "127.0.0.1".to_string(),
            cors_enabled: false,
        }
    }
}

/// Start the HTTP server
pub async fn start_server(config: ServerConfig) -> anyhow::Result<()> {
    // Load safe-coder config
    let app_config = Config::load().unwrap_or_default();

    // Create shared state with persistence
    let state = match AppState::new_with_persistence(app_config.clone()).await {
        Ok(state) => {
            tracing::info!("Session persistence enabled (SQLite)");
            Arc::new(state)
        }
        Err(e) => {
            tracing::warn!("Failed to enable persistence: {}. Using in-memory storage.", e);
            Arc::new(AppState::new(app_config))
        }
    };

    // Build router
    let mut app = Router::new()
        // Health & config
        .route("/api/health", get(routes::health::health_check))
        .route("/api/config", get(routes::config::get_config))

        // Sessions
        .route("/api/sessions", get(routes::sessions::list_sessions))
        .route("/api/sessions", post(routes::sessions::create_session))
        .route("/api/sessions/:id", get(routes::sessions::get_session))
        .route("/api/sessions/:id", delete(routes::sessions::delete_session))
        .route("/api/sessions/:id/mode", put(routes::sessions::set_session_mode))

        // Messages
        .route("/api/sessions/:id/messages", get(routes::messages::get_messages))
        .route("/api/sessions/:id/messages", post(routes::messages::send_message))
        .route("/api/sessions/:id/cancel", post(routes::messages::cancel_operation))
        .route("/api/sessions/:id/doom-loop-response", post(routes::sessions::respond_to_doom_loop))

        // File changes
        .route("/api/sessions/:id/changes", get(routes::files::get_session_changes))
        .route("/api/sessions/:id/files", get(routes::files::list_project_files))

        // Real-time events (SSE)
        .route("/api/sessions/:id/events", get(routes::events::session_events))

        // PTY WebSocket
        .route("/api/sessions/:id/pty", get(routes::pty::pty_websocket))

        // OpenAPI docs
        .route("/api/openapi.json", get(routes::openapi::openapi_spec))

        .with_state(state)
        .layer(TraceLayer::new_for_http());

    // Add CORS if enabled
    if config.cors_enabled {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
        app = app.layer(cors);
    }

    // Bind and serve
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid address: {}", e))?;

    println!("Starting safe-coder server on http://{}", addr);
    println!("API documentation: http://{}/api/openapi.json", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
