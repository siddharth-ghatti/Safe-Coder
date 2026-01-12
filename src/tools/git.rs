use serde::Deserialize;
use anyhow::Result;
use async_trait::async_trait;
use crate::tools::{Tool, ToolContext};
use crate::git::GitManager;

#[derive(Deserialize)]
pub struct GitParams {
    pub command: String, // e.g. "status", "diff", "log", "undo", "redo", "commit"  
    pub message: Option<String>, // Only for commit/snapshot
    pub log_count: Option<usize>, // Only for log
}

pub struct GitTool;

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &str {
        "git"
    }
    fn description(&self) -> &str {
        "Git tool: status, diff, log, undo, redo, commit, snapshot. Params: command, message, log_count."
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "message": {"type": "string"},
                "log_count": {"type": "integer"}
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let parsed: GitParams = serde_json::from_value(params)?;
        let repo_path = ctx.working_dir.to_path_buf();
        let mut git_mgr = GitManager::new(repo_path);
        match parsed.command.as_str() {
            "status" => git_mgr.status().await,
            "diff" => git_mgr.diff().await,
            "log" => git_mgr.log(parsed.log_count.unwrap_or(10)).await,
            "undo" => {
                let res = git_mgr.undo().await?;
                Ok(format!("Undone to prev commit: {}\nFiles: {:?}", res.commit_undone, res.files_restored))
            },
            "redo" => {
                let res = git_mgr.redo().await?;
                Ok(format!("Redone commit: {}\nFiles: {:?}", res.commit_restored, res.files_restored))
            },
            "commit" => {
                let msg = parsed.message.as_deref().unwrap_or("Safe Coder auto-commit");
                git_mgr.auto_commit(msg).await?;
                Ok("Committed changes".to_string())
            },
            "snapshot" => {
                let msg = parsed.message.as_deref().unwrap_or("Snapshot");
                git_mgr.snapshot(msg).await?;
                Ok("Snapshot committed".to_string())
            }
            _ => Err(anyhow::anyhow!("Unknown git command")),
        }
    }
}
