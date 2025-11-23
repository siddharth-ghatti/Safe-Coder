use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

pub mod github_copilot;
pub mod anthropic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub token_type: String,
}

#[derive(Debug, Clone)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
}

impl StoredToken {
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            now >= expires_at
        } else {
            false
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read token file")?;
        let token: StoredToken = serde_json::from_str(&content)
            .context("Failed to parse token file")?;
        Ok(token)
    }
}

#[async_trait::async_trait]
pub trait DeviceFlowAuth: Send + Sync {
    async fn start_device_flow(&self) -> Result<DeviceCodeResponse>;
    async fn poll_for_token(&self, device_code: &str, interval: u64) -> Result<StoredToken>;
}

pub async fn run_device_flow<T: DeviceFlowAuth>(
    auth: &T,
    provider_name: &str,
) -> Result<StoredToken> {
    println!("\n🔐 Starting {} authentication...\n", provider_name);

    let device_response = auth.start_device_flow().await?;

    println!("📋 Please visit: {}", device_response.verification_uri);
    println!("🔑 Enter code: {}\n", device_response.user_code);

    if let Some(complete_uri) = &device_response.verification_uri_complete {
        println!("Or open this URL directly:");
        println!("🔗 {}\n", complete_uri);
    }

    println!("⏳ Waiting for authorization...");

    let token = auth.poll_for_token(
        &device_response.device_code,
        device_response.interval,
    ).await?;

    println!("✅ Successfully authenticated!\n");

    Ok(token)
}
