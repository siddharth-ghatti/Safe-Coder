use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

use super::{DeviceCodeResponse, DeviceFlowAuth, StoredToken};

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98"; // GitHub Copilot CLI client ID

pub struct GitHubCopilotAuth {
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct DeviceCodeRequest {
    client_id: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct GitHubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
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
        scope: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<u64>,
    },
    Pending {
        error: String,
    },
}

impl GitHubCopilotAuth {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl DeviceFlowAuth for GitHubCopilotAuth {
    async fn start_device_flow(&self) -> Result<DeviceCodeResponse> {
        let request = DeviceCodeRequest {
            client_id: GITHUB_CLIENT_ID.to_string(),
            scope: "read:user copilot".to_string(),
        };

        let response = self
            .client
            .post(GITHUB_DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&request)
            .send()
            .await
            .context("Failed to request device code")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("GitHub device code request failed ({}): {}", status, text);
        }

        let github_response: GitHubDeviceCodeResponse = response
            .json()
            .await
            .context("Failed to parse device code response")?;

        let complete_uri = format!(
            "{}/{}",
            github_response.verification_uri, github_response.user_code
        );

        Ok(DeviceCodeResponse {
            device_code: github_response.device_code,
            user_code: github_response.user_code,
            verification_uri: github_response.verification_uri,
            verification_uri_complete: Some(complete_uri),
            expires_in: github_response.expires_in,
            interval: github_response.interval,
        })
    }

    async fn poll_for_token(&self, device_code: &str, interval: u64) -> Result<StoredToken> {
        let request = TokenRequest {
            client_id: GITHUB_CLIENT_ID.to_string(),
            device_code: device_code.to_string(),
            grant_type: "urn:ietf:params:oauth:grant-type:device_code".to_string(),
        };

        loop {
            sleep(Duration::from_secs(interval)).await;

            let response = self
                .client
                .post(GITHUB_TOKEN_URL)
                .header("Accept", "application/json")
                .form(&request)
                .send()
                .await
                .context("Failed to poll for token")?;

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await?;
                anyhow::bail!("GitHub token request failed ({}): {}", status, text);
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
                    ..
                } => {
                    let expires_at = expires_in.map(|exp| {
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            + exp
                    });

                    return Ok(StoredToken::Device {
                        access_token,
                        refresh_token,
                        expires_at,
                        token_type,
                    });
                }
                TokenResponse::Pending { error } => {
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
                        anyhow::bail!("Unknown error: {}", error);
                    }
                }
            }
        }
    }
}

// Helper function to get Copilot token from GitHub token
pub async fn get_copilot_token(github_token: &str) -> Result<String> {
    let client = reqwest::Client::new();

    // GitHub Copilot token endpoint
    let response = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", github_token))
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to get Copilot token")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get Copilot token ({}): {}", status, text);
    }

    #[derive(Deserialize)]
    struct CopilotTokenResponse {
        token: String,
    }

    let copilot_response: CopilotTokenResponse = response
        .json()
        .await
        .context("Failed to parse Copilot token response")?;

    Ok(copilot_response.token)
}
