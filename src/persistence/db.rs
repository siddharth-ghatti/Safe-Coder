use super::models::SavedSession;
use anyhow::{Context, Result};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::path::PathBuf;

/// SQLite database for session persistence
pub struct SessionDatabase {
    pool: SqlitePool,
}

impl SessionDatabase {
    /// Create new database connection
    pub async fn new() -> Result<Self> {
        let db_path = Self::db_path()?;

        // Create directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Use ?mode=rwc to create the database file if it doesn't exist
        let database_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .context("Failed to connect to database")?;

        // Run migrations
        Self::migrate(&pool).await?;

        Ok(Self { pool })
    }

    /// Get database file path
    fn db_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;
        Ok(config_dir.join("safe-coder").join("sessions.db"))
    }

    /// Run database migrations
    async fn migrate(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                name TEXT,
                project_path TEXT NOT NULL,
                messages TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Create index for faster lookups
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sessions_created_at
            ON sessions(created_at DESC)
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Save a session
    pub async fn save_session(&self, session: &SavedSession) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, name, project_path, messages, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.project_path)
        .bind(&session.messages)
        .bind(session.created_at.to_rfc3339())
        .bind(session.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get a session by ID
    pub async fn get_session(&self, id: &str) -> Result<SavedSession> {
        let row = sqlx::query_as::<_, (String, Option<String>, String, String, String, String)>(
            r#"
            SELECT id, name, project_path, messages, created_at, updated_at
            FROM sessions
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .context("Session not found")?;

        Ok(SavedSession {
            id: row.0,
            name: row.1,
            project_path: row.2,
            messages: row.3,
            created_at: row.4.parse().context("Invalid created_at")?,
            updated_at: row.5.parse().context("Invalid updated_at")?,
        })
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Result<Vec<SavedSession>> {
        let rows = sqlx::query_as::<_, (String, Option<String>, String, String, String, String)>(
            r#"
            SELECT id, name, project_path, messages, created_at, updated_at
            FROM sessions
            ORDER BY updated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let sessions = rows
            .into_iter()
            .map(|row| {
                Ok(SavedSession {
                    id: row.0,
                    name: row.1,
                    project_path: row.2,
                    messages: row.3,
                    created_at: row.4.parse()?,
                    updated_at: row.5.parse()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(sessions)
    }

    /// Delete a session
    pub async fn delete_session(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Update session messages
    pub async fn update_session_messages(&self, id: &str, messages: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET messages = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(messages)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
