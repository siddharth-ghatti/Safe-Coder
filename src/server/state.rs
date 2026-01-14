//! Server state management
//!
//! This module defines the shared state for the HTTP server, including
//! session management and event broadcasting.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, RwLock};

use crate::config::Config;
use crate::persistence::SessionPersistence;
use crate::session::Session;

use super::types::ServerEvent;

/// Shared application state for the server
pub struct AppState {
    /// Application configuration
    pub config: RwLock<Config>,

    /// Active sessions mapped by ID
    pub sessions: RwLock<HashMap<String, SessionHandle>>,

    /// Event broadcast channels per session
    pub event_channels: RwLock<HashMap<String, broadcast::Sender<ServerEvent>>>,

    /// Session persistence layer (SQLite)
    pub persistence: Option<Arc<SessionPersistence>>,

    /// Pending doom loop response channels (prompt_id -> response sender)
    pub doom_loop_responses: RwLock<HashMap<String, mpsc::UnboundedSender<bool>>>,
}

/// Handle to a managed session
pub struct SessionHandle {
    /// The session instance
    pub session: Arc<RwLock<Session>>,

    /// Project path for this session
    pub project_path: PathBuf,

    /// Session creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Whether the session is currently processing
    pub is_processing: Arc<RwLock<bool>>,

    /// Tracked file changes in this session
    pub file_changes: Arc<RwLock<Vec<FileChange>>>,
}

/// Represents a file change in a session
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: FileChangeType,
    pub additions: i32,
    pub deletions: i32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

/// Type of file change
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileChangeType {
    Created,
    Modified,
    Deleted,
}

impl AppState {
    /// Create new application state
    pub fn new(config: Config) -> Self {
        Self {
            config: RwLock::new(config),
            sessions: RwLock::new(HashMap::new()),
            event_channels: RwLock::new(HashMap::new()),
            persistence: None,
            doom_loop_responses: RwLock::new(HashMap::new()),
        }
    }

    /// Create new application state with persistence
    pub async fn new_with_persistence(config: Config) -> anyhow::Result<Self> {
        let persistence = SessionPersistence::new().await?;
        Ok(Self {
            config: RwLock::new(config),
            sessions: RwLock::new(HashMap::new()),
            event_channels: RwLock::new(HashMap::new()),
            persistence: Some(Arc::new(persistence)),
            doom_loop_responses: RwLock::new(HashMap::new()),
        })
    }

    /// Register a doom loop response channel
    pub async fn register_doom_loop_response(&self, prompt_id: String, response_tx: mpsc::UnboundedSender<bool>) {
        let mut responses = self.doom_loop_responses.write().await;
        responses.insert(prompt_id, response_tx);
    }

    /// Send a doom loop response
    pub async fn send_doom_loop_response(&self, prompt_id: &str, continue_anyway: bool) -> bool {
        let mut responses = self.doom_loop_responses.write().await;
        if let Some(tx) = responses.remove(prompt_id) {
            tx.send(continue_anyway).is_ok()
        } else {
            false
        }
    }

    /// Get the persistence layer
    pub fn persistence(&self) -> Option<&Arc<SessionPersistence>> {
        self.persistence.as_ref()
    }

    /// Get or create an event channel for a session
    pub async fn get_event_sender(&self, session_id: &str) -> broadcast::Sender<ServerEvent> {
        let mut channels = self.event_channels.write().await;

        if let Some(sender) = channels.get(session_id) {
            sender.clone()
        } else {
            let (sender, _) = broadcast::channel(1024);
            channels.insert(session_id.to_string(), sender.clone());
            sender
        }
    }

    /// Subscribe to events for a session
    pub async fn subscribe_events(&self, session_id: &str) -> broadcast::Receiver<ServerEvent> {
        let sender = self.get_event_sender(session_id).await;
        sender.subscribe()
    }

    /// Broadcast an event to all subscribers
    pub async fn broadcast_event(&self, session_id: &str, event: ServerEvent) {
        let channels = self.event_channels.read().await;
        if let Some(sender) = channels.get(session_id) {
            let _ = sender.send(event);
        }
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<SessionHandle> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// Check if a session exists
    pub async fn session_exists(&self, session_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.contains_key(session_id)
    }

    /// Add a file change to a session
    pub async fn add_file_change(&self, session_id: &str, change: FileChange) {
        let sessions = self.sessions.read().await;
        if let Some(handle) = sessions.get(session_id) {
            let mut changes = handle.file_changes.write().await;

            // Update existing change for the same path or add new
            if let Some(existing) = changes.iter_mut().find(|c| c.path == change.path) {
                existing.additions += change.additions;
                existing.deletions += change.deletions;
                existing.timestamp = change.timestamp;
                existing.new_content = change.new_content;
            } else {
                changes.push(change);
            }
        }
    }

    /// Get all file changes for a session
    pub async fn get_file_changes(&self, session_id: &str) -> Vec<FileChange> {
        let sessions = self.sessions.read().await;
        if let Some(handle) = sessions.get(session_id) {
            let changes = handle.file_changes.read().await;
            changes.clone()
        } else {
            Vec::new()
        }
    }
}

impl Clone for SessionHandle {
    fn clone(&self) -> Self {
        Self {
            session: Arc::clone(&self.session),
            project_path: self.project_path.clone(),
            created_at: self.created_at,
            is_processing: Arc::clone(&self.is_processing),
            file_changes: Arc::clone(&self.file_changes),
        }
    }
}
