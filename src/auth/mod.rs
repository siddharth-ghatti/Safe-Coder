use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

pub mod github_copilot;
pub mod anthropic;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StoredToken {
    /// API key based authentication
    #[serde(rename = "api")]
    Api {
        key: String,
    },
    /// OAuth token with refresh capability
    #[serde(rename = "oauth")]
    OAuth {
        access_token: String,
        refresh_token: String,
        expires_at: u64,
    },
    /// Legacy device flow token (for GitHub Copilot)
    #[serde(rename = "device")]
    Device {
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<u64>,
        token_type: String,
    },
}

impl StoredToken {
    pub fn is_expired(&self) -> bool {
        match self {
            StoredToken::Api { .. } => false,
            StoredToken::OAuth { expires_at, .. } => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                now >= *expires_at
            }
            StoredToken::Device { expires_at, .. } => {
                if let Some(exp) = expires_at {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    now >= *exp
                } else {
                    false
                }
            }
        }
    }

    pub fn get_access_token(&self) -> &str {
        match self {
            StoredToken::Api { key } => key,
            StoredToken::OAuth { access_token, .. } => access_token,
            StoredToken::Device { access_token, .. } => access_token,
        }
    }

    pub fn is_oauth(&self) -> bool {
        matches!(self, StoredToken::OAuth { .. })
    }

    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        // Set restrictive permissions on the token file
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
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

#[derive(Debug, Clone)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: u64,
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
    println!("\n Starting {} authentication...\n", provider_name);

    let device_response = auth.start_device_flow().await?;

    println!("Please visit: {}", device_response.verification_uri);
    println!("Enter code: {}\n", device_response.user_code);

    if let Some(complete_uri) = &device_response.verification_uri_complete {
        println!("Or open this URL directly:");
        println!("{}\n", complete_uri);
    }

    println!("Waiting for authorization...");

    let token = auth.poll_for_token(
        &device_response.device_code,
        device_response.interval,
    ).await?;

    println!("Successfully authenticated!\n");

    Ok(token)
}

/// PKCE utilities for OAuth
pub mod pkce {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use rand::Rng;
    use sha2::{Digest, Sha256};

    pub struct PkceChallenge {
        pub verifier: String,
        pub challenge: String,
    }

    pub fn generate() -> PkceChallenge {
        // Generate a random 32-byte verifier
        let mut rng = rand::thread_rng();
        let random_bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let verifier = URL_SAFE_NO_PAD.encode(&random_bytes);

        // SHA256 hash of verifier, then base64url encode
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        let challenge = URL_SAFE_NO_PAD.encode(hash);

        PkceChallenge { verifier, challenge }
    }
}
