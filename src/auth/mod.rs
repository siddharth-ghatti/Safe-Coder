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

/// Buffer time before expiry to trigger refresh (5 minutes in milliseconds)
const REFRESH_BUFFER_MS: u64 = 5 * 60 * 1000;

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

    /// Check if token will expire soon (within 5 minutes) and should be refreshed
    pub fn needs_refresh(&self) -> bool {
        match self {
            StoredToken::Api { .. } => false,
            StoredToken::OAuth { expires_at, .. } => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                // Refresh if within 5 minutes of expiry
                now + REFRESH_BUFFER_MS >= *expires_at
            }
            StoredToken::Device { expires_at, .. } => {
                if let Some(exp) = expires_at {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    // Refresh if within 5 minutes of expiry
                    now + (REFRESH_BUFFER_MS / 1000) >= *exp
                } else {
                    false
                }
            }
        }
    }

    /// Get the refresh token if available
    pub fn get_refresh_token(&self) -> Option<&str> {
        match self {
            StoredToken::Api { .. } => None,
            StoredToken::OAuth { refresh_token, .. } => Some(refresh_token),
            StoredToken::Device { refresh_token, .. } => refresh_token.as_deref(),
        }
    }

    /// Get time until expiry in seconds (None if no expiry)
    pub fn seconds_until_expiry(&self) -> Option<i64> {
        match self {
            StoredToken::Api { .. } => None,
            StoredToken::OAuth { expires_at, .. } => {
                let now = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                Some(((*expires_at as i64) - (now as i64)) / 1000)
            }
            StoredToken::Device { expires_at, .. } => {
                expires_at.map(|exp| {
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    (exp as i64) - (now as i64)
                })
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

    /// Check if this token type supports refresh
    pub fn supports_refresh(&self) -> bool {
        match self {
            StoredToken::Api { .. } => false,
            StoredToken::OAuth { .. } => true,
            StoredToken::Device { refresh_token, .. } => refresh_token.is_some(),
        }
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

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages token lifecycle including automatic refresh
///
/// This manager handles:
/// - Checking if tokens need refresh before expiry
/// - Automatically refreshing tokens
/// - Saving refreshed tokens to disk
/// - Thread-safe access to current token
pub struct TokenManager {
    /// Current token (wrapped in RwLock for thread-safe access)
    token: Arc<RwLock<StoredToken>>,
    /// Path to save token updates
    token_path: PathBuf,
    /// Provider for refresh operations
    provider: TokenProvider,
}

impl std::fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenManager")
            .field("token_path", &self.token_path)
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

/// Provider-specific refresh capability
#[derive(Debug, Clone)]
pub enum TokenProvider {
    Anthropic,
    GitHubCopilot,
    /// No refresh capability (API key only)
    None,
}

impl TokenManager {
    /// Create a new token manager
    pub fn new(token: StoredToken, token_path: PathBuf, provider: TokenProvider) -> Self {
        Self {
            token: Arc::new(RwLock::new(token)),
            token_path,
            provider,
        }
    }

    /// Get the current access token, refreshing if needed
    ///
    /// This is the main entry point - it will:
    /// 1. Check if the current token needs refresh
    /// 2. If so, refresh it and save to disk
    /// 3. Return the current (possibly refreshed) access token
    pub async fn get_valid_token(&self) -> Result<String> {
        // First, check if we need to refresh
        let needs_refresh = {
            let token = self.token.read().await;
            token.needs_refresh() && token.supports_refresh()
        };

        if needs_refresh {
            self.refresh().await?;
        }

        // Return the current access token
        let token = self.token.read().await;
        Ok(token.get_access_token().to_string())
    }

    /// Get the current token (without refresh check)
    pub async fn get_current_token(&self) -> StoredToken {
        self.token.read().await.clone()
    }

    /// Check if token is expired or will expire soon
    pub async fn needs_refresh(&self) -> bool {
        let token = self.token.read().await;
        token.needs_refresh()
    }

    /// Check if token is already expired
    pub async fn is_expired(&self) -> bool {
        let token = self.token.read().await;
        token.is_expired()
    }

    /// Get seconds until token expiry
    pub async fn seconds_until_expiry(&self) -> Option<i64> {
        let token = self.token.read().await;
        token.seconds_until_expiry()
    }

    /// Manually trigger a token refresh
    pub async fn refresh(&self) -> Result<()> {
        let current_token = self.token.read().await.clone();

        // Get refresh token
        let refresh_token = current_token
            .get_refresh_token()
            .context("Token does not support refresh")?;

        // Perform refresh based on provider
        let new_token = match &self.provider {
            TokenProvider::Anthropic => {
                let auth = anthropic::AnthropicAuth::new();
                auth.refresh_token(refresh_token).await?
            }
            TokenProvider::GitHubCopilot => {
                // GitHub Copilot refresh not yet implemented
                anyhow::bail!("GitHub Copilot token refresh not yet implemented")
            }
            TokenProvider::None => {
                anyhow::bail!("This token type does not support refresh")
            }
        };

        // Update the stored token
        {
            let mut token = self.token.write().await;
            *token = new_token.clone();
        }

        // Save to disk
        new_token.save(&self.token_path)
            .context("Failed to save refreshed token")?;

        tracing::info!("Token refreshed successfully");

        if let Some(secs) = new_token.seconds_until_expiry() {
            tracing::debug!("New token expires in {} seconds", secs);
        }

        Ok(())
    }

    /// Update the token (e.g., after initial auth)
    pub async fn update_token(&self, token: StoredToken) -> Result<()> {
        // Update in memory
        {
            let mut current = self.token.write().await;
            *current = token.clone();
        }

        // Save to disk
        token.save(&self.token_path)
            .context("Failed to save token")?;

        Ok(())
    }
}
