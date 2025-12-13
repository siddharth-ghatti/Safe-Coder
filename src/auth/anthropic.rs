use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

use super::{pkce, StoredToken};

// Claude Code OAuth client ID (from OpenCode)
const ANTHROPIC_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
const ANTHROPIC_TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
const ANTHROPIC_REDIRECT_URI: &str = "https://console.anthropic.com/oauth/code/callback";

/// Authentication mode for Anthropic
#[derive(Debug, Clone, Copy)]
pub enum AuthMode {
    /// Claude Pro/Max subscription - uses claude.ai
    ClaudeMax,
    /// Console API - creates an API key
    Console,
}

impl AuthMode {
    fn authorization_url(&self) -> &'static str {
        match self {
            AuthMode::ClaudeMax => "https://claude.ai/oauth/authorize",
            AuthMode::Console => "https://console.anthropic.com/oauth/authorize",
        }
    }
}

pub struct AnthropicAuth {
    client: reqwest::Client,
}

/// Pending authorization state
pub struct PendingAuthorization {
    pub url: String,
    pub verifier: String,
    pub mode: AuthMode,
}

#[derive(Debug, Serialize)]
struct TokenExchangeRequest {
    code: String,
    state: String,
    grant_type: String,
    client_id: String,
    redirect_uri: String,
    code_verifier: String,
}

#[derive(Debug, Serialize)]
struct TokenRefreshRequest {
    grant_type: String,
    refresh_token: String,
    client_id: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiKeyResponse {
    raw_key: String,
}

impl AnthropicAuth {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Start the OAuth authorization flow
    /// Returns the URL to visit and the PKCE verifier to use for token exchange
    pub fn start_authorization(&self, mode: AuthMode) -> PendingAuthorization {
        let pkce_challenge = pkce::generate();

        let mut url = url::Url::parse(mode.authorization_url()).unwrap();
        url.query_pairs_mut()
            .append_pair("code", "true")
            .append_pair("client_id", ANTHROPIC_CLIENT_ID)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", ANTHROPIC_REDIRECT_URI)
            .append_pair("scope", "org:create_api_key user:profile user:inference")
            .append_pair("code_challenge", &pkce_challenge.challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &pkce_challenge.verifier);

        PendingAuthorization {
            url: url.to_string(),
            verifier: pkce_challenge.verifier,
            mode,
        }
    }

    /// Exchange the authorization code for tokens
    /// The code format is: "authorization_code#state"
    pub async fn exchange_code(
        &self,
        code: &str,
        verifier: &str,
        mode: AuthMode,
    ) -> Result<StoredToken> {
        // Parse the code - format is "code#state"
        let parts: Vec<&str> = code.split('#').collect();
        let (auth_code, state) = if parts.len() == 2 {
            (parts[0], parts[1])
        } else {
            (code, verifier)
        };

        let request = TokenExchangeRequest {
            code: auth_code.to_string(),
            state: state.to_string(),
            grant_type: "authorization_code".to_string(),
            client_id: ANTHROPIC_CLIENT_ID.to_string(),
            redirect_uri: ANTHROPIC_REDIRECT_URI.to_string(),
            code_verifier: verifier.to_string(),
        };

        let response = self.client
            .post(ANTHROPIC_TOKEN_URL)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to exchange code for token")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Token exchange failed ({}): {}", status, text);
        }

        let token_response: TokenResponse = response.json().await
            .context("Failed to parse token response")?;

        // For Console mode, we need to create an API key
        if matches!(mode, AuthMode::Console) {
            return self.create_api_key(&token_response.access_token).await;
        }

        // For Claude Max mode, return OAuth tokens
        let expires_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + (token_response.expires_in * 1000);

        Ok(StoredToken::OAuth {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
        })
    }

    /// Create an API key using the access token (Console mode)
    async fn create_api_key(&self, access_token: &str) -> Result<StoredToken> {
        let response = self.client
            .post("https://api.anthropic.com/api/oauth/claude_cli/create_api_key")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .context("Failed to create API key")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("API key creation failed ({}): {}", status, text);
        }

        let api_key_response: ApiKeyResponse = response.json().await
            .context("Failed to parse API key response")?;

        Ok(StoredToken::Api {
            key: api_key_response.raw_key,
        })
    }

    /// Refresh an OAuth token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<StoredToken> {
        let request = TokenRefreshRequest {
            grant_type: "refresh_token".to_string(),
            refresh_token: refresh_token.to_string(),
            client_id: ANTHROPIC_CLIENT_ID.to_string(),
        };

        let response = self.client
            .post(ANTHROPIC_TOKEN_URL)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to refresh token")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Token refresh failed ({}): {}", status, text);
        }

        let token_response: TokenResponse = response.json().await
            .context("Failed to parse refresh token response")?;

        let expires_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + (token_response.expires_in * 1000);

        Ok(StoredToken::OAuth {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
        })
    }
}

/// Get the beta headers for OAuth-authenticated requests
pub fn get_oauth_beta_headers() -> String {
    [
        "oauth-2025-04-20",
        "claude-code-20250219",
        "interleaved-thinking-2025-05-14",
        "fine-grained-tool-streaming-2025-05-14",
    ].join(",")
}
