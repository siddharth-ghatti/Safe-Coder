mod db;
pub mod event_log;
pub mod models;

pub use db::SessionDatabase;
pub use event_log::{EventLogger, SessionInfo, SessionLogEvent};
pub use models::{SavedSession, SessionStats, ToolUsage};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::llm::Message;

/// Session persistence manager
pub struct SessionPersistence {
    db: SessionDatabase,
}

impl SessionPersistence {
    /// Create new persistence manager
    pub async fn new() -> Result<Self> {
        let db = SessionDatabase::new().await?;
        Ok(Self { db })
    }

    /// Save a chat session
    pub async fn save_session(
        &self,
        name: Option<String>,
        project_path: &PathBuf,
        messages: &[Message],
    ) -> Result<String> {
        let session = SavedSession {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            project_path: project_path.to_string_lossy().to_string(),
            messages: serde_json::to_string(messages)?,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.db.save_session(&session).await?;
        Ok(session.id)
    }

    /// Resume a chat session
    pub async fn resume_session(&self, id: &str) -> Result<SavedSession> {
        self.db.get_session(id).await
    }

    /// List all saved sessions
    pub async fn list_sessions(&self) -> Result<Vec<SavedSession>> {
        self.db.list_sessions().await
    }

    /// Delete a session
    pub async fn delete_session(&self, id: &str) -> Result<()> {
        self.db.delete_session(id).await
    }

    /// Update session messages
    pub async fn update_session(&self, id: &str, messages: &[Message]) -> Result<()> {
        let messages_json = serde_json::to_string(messages)?;
        self.db.update_session_messages(id, &messages_json).await
    }
}
