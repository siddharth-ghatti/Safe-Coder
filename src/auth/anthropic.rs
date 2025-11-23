use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

use super::{DeviceCodeResponse, DeviceFlowAuth, StoredToken};

// Note: These endpoints are based on standard OAuth 2.0 device flow
// Anthropic may use different endpoints for Claude.ai authentication
const ANTHROPIC_DEVICE_CODE_URL: &str = "https://api.anthropic.com/v1/oauth/device/code";
const ANTHROPIC_TOKEN_URL: &str = "https://api.anthropic.com/v1/oauth/token";
const ANTHROPIC_CLIENT_ID: &str = "anthropic-cli"; // This may need to be updated

pub struct AnthropicAuth {
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct DeviceCodeRequest {
    client_id: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: u64,
}

#[derive(Debug, Serialize)]
struct TokenRequest {
    client_id: String,
    device_code: String,
    grant_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TokenResponse {
    Success {
        access_token: String,
        token_type: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<u64>,
    },
    Pending {
        error: String,
        #[serde(default)]
        error_description: Option<String>,
    },
}

impl AnthropicAuth {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl DeviceFlowAuth for AnthropicAuth {
    async fn start_device_flow(&self) -> Result<DeviceCodeResponse> {
        let request = DeviceCodeRequest {
            client_id: ANTHROPIC_CLIENT_ID.to_string(),
            scope: "api".to_string(),
        };

        let response = self
            .client
            .post(ANTHROPIC_DEVICE_CODE_URL)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to request device code from Anthropic")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Anthropic device code request failed ({}): {}", status, text);
        }

        let anthropic_response: AnthropicDeviceCodeResponse = response
            .json()
            .await
            .context("Failed to parse device code response")?;

        Ok(DeviceCodeResponse {
            device_code: anthropic_response.device_code,
            user_code: anthropic_response.user_code,
            verification_uri: anthropic_response.verification_uri,
            verification_uri_complete: anthropic_response.verification_uri_complete,
            expires_in: anthropic_response.expires_in,
            interval: anthropic_response.interval,
        })
    }

    async fn poll_for_token(&self, device_code: &str, interval: u64) -> Result<StoredToken> {
        let request = TokenRequest {
            client_id: ANTHROPIC_CLIENT_ID.to_string(),
            device_code: device_code.to_string(),
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
        };

        loop {
            sleep(Duration::from_secs(interval)).await;

            let response = self
                .client
                .post(ANTHROPIC_TOKEN_URL)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(&request)
                .send()
                .await
                .context("Failed to poll for token")?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await?;
                anyhow::bail!("Anthropic token request failed ({}): {}", status, text);
            }

            let token_response: TokenResponse = response
                .json()
                .await
                .context("Failed to parse token response")?;

            match token_response {
                TokenResponse::Success {
                    access_token,
                    token_type,
                    refresh_token,
                    expires_in,
                } => {
                    let expires_at = expires_in.map(|exp| {
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            + exp
                    });

                    return Ok(StoredToken {
                        access_token,
                        refresh_token,
                        expires_at,
                        token_type,
                    });
                }
                TokenResponse::Pending { error, .. } => {
                    if error == "authorization_pending" {
                        // Continue polling
                        continue;
                    } else if error == "slow_down" {
                        // Increase interval
                        sleep(Duration::from_secs(5)).await;
                        continue;
                    } else if error == "expired_token" {
                        anyhow::bail!("Device code expired. Please try again.");
                    } else if error == "access_denied" {
                        anyhow::bail!("Access denied by user.");
                    } else {
                        anyhow::bail!("Authentication error: {}", error);
                    }
                }
            }
        }
    }
}

// Helper to get session token from Claude.ai (alternative auth method)
pub async fn get_claude_session_token(email: &str) -> Result<String> {
    // This is a placeholder for Claude.ai session-based authentication
    // The actual implementation would depend on Anthropic's consumer API
    println!("Note: Claude.ai session authentication is not yet fully implemented.");
    println!("Please use an API key from https://console.anthropic.com for now.");
    anyhow::bail!("Claude.ai session auth not implemented. Use API key instead.")
}
