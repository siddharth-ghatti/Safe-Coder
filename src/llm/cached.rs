//! Caching wrapper for LLM clients
//!
//! This module provides a decorator that wraps any LlmClient implementation
//! to add caching capabilities. It works with any provider (Anthropic, OpenAI,
//! Ollama, GitHub Copilot) through the LlmClient trait.

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use super::{LlmClient, LlmResponse, Message, TokenUsage, ToolDefinition};
use crate::cache::{CacheKey, CacheStats, CacheStore, CachedResponse, MemoryCache};

/// Configuration for the caching LLM client
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Whether to use provider-native caching (Anthropic cache_control, etc.)
    /// Note: This is handled by the underlying client, not this wrapper
    pub provider_native: bool,
    /// Whether to use application-level response caching
    pub application_cache: bool,
    /// Default TTL for cached responses
    pub ttl: Duration,
    /// Maximum cache entries
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider_native: true,
            application_cache: true,
            ttl: Duration::from_secs(30 * 60), // 30 minutes
            max_entries: 100,
        }
    }
}

impl CacheConfig {
    /// Create a config with caching disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a config with only provider-native caching
    pub fn provider_only() -> Self {
        Self {
            enabled: true,
            provider_native: true,
            application_cache: false,
            ..Default::default()
        }
    }
}

/// A caching wrapper for any LLM client
///
/// This decorator implements the LlmClient trait and wraps another client,
/// adding caching capabilities. It checks the cache before making API calls
/// and stores responses for future use.
pub struct CachingLlmClient {
    /// The underlying LLM client
    inner: Box<dyn LlmClient>,
    /// Cache storage
    cache: Arc<dyn CacheStore>,
    /// Configuration
    config: CacheConfig,
    /// Model name (for cache keys)
    model: String,
}

impl CachingLlmClient {
    /// Create a new caching client wrapper
    pub fn new(inner: Box<dyn LlmClient>, model: String, config: CacheConfig) -> Self {
        let cache: Arc<dyn CacheStore> = Arc::new(MemoryCache::new(config.max_entries, config.ttl));

        Self {
            inner,
            cache,
            config,
            model,
        }
    }

    /// Create a new caching client with a custom cache store
    pub fn with_cache(
        inner: Box<dyn LlmClient>,
        model: String,
        config: CacheConfig,
        cache: Arc<dyn CacheStore>,
    ) -> Self {
        Self {
            inner,
            cache,
            config,
            model,
        }
    }

    /// Get cache statistics
    pub async fn cache_stats(&self) -> CacheStats {
        self.cache.stats().await
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    /// Check if caching is enabled
    pub fn is_caching_enabled(&self) -> bool {
        self.config.enabled && self.config.application_cache
    }
}

#[async_trait]
impl LlmClient for CachingLlmClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse> {
        // If caching is disabled, just pass through
        if !self.is_caching_enabled() {
            return self
                .inner
                .send_message_with_system(messages, tools, system_prompt)
                .await;
        }

        // Generate cache key
        let cache_key = CacheKey::from_request(system_prompt, messages, tools, &self.model);

        // Check cache first
        if let Some(cached) = self.cache.get(&cache_key).await {
            tracing::debug!(
                "Cache hit: returning cached response (saved {} input tokens)",
                cached
                    .response
                    .usage
                    .as_ref()
                    .map(|u| u.input_tokens)
                    .unwrap_or(0)
            );

            // Return cached response with updated usage to indicate cache hit
            let mut response = cached.response;
            if let Some(ref mut usage) = response.usage {
                // Mark all input tokens as cache reads since we didn't call the API
                usage.cache_read_tokens = Some(usage.input_tokens);
            }
            return Ok(response);
        }

        // Cache miss - call the underlying client
        tracing::debug!("Cache miss: calling LLM provider");
        let response = self
            .inner
            .send_message_with_system(messages, tools, system_prompt)
            .await?;

        // Only cache responses without tool calls (tool calls are dynamic)
        let has_tool_calls = response
            .message
            .content
            .iter()
            .any(|block| matches!(block, super::ContentBlock::ToolUse { .. }));

        if !has_tool_calls {
            // Cache the response
            let cached_response = CachedResponse::new(response.clone(), self.config.ttl);
            self.cache.set(&cache_key, cached_response).await;
            tracing::debug!("Cached response for future use");
        } else {
            tracing::debug!("Not caching response with tool calls");
        }

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ContentBlock, Message, Role};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A mock LLM client for testing
    struct MockLlmClient {
        call_count: Arc<AtomicUsize>,
        response: LlmResponse,
    }

    impl MockLlmClient {
        fn new() -> Self {
            Self {
                call_count: Arc::new(AtomicUsize::new(0)),
                response: LlmResponse {
                    message: Message {
                        role: Role::Assistant,
                        content: vec![ContentBlock::Text {
                            text: "Test response".to_string(),
                        }],
                    },
                    usage: Some(TokenUsage::new(100, 50)),
                },
            }
        }

        fn call_count(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn send_message_with_system(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system_prompt: Option<&str>,
        ) -> Result<LlmResponse> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn test_caching_prevents_duplicate_calls() {
        let mock = MockLlmClient::new();
        let call_count = mock.call_count.clone();

        let client = CachingLlmClient::new(
            Box::new(mock),
            "test-model".to_string(),
            CacheConfig::default(),
        );

        let messages = vec![Message::user("Hello".to_string())];

        // First call - should hit the underlying client
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call with same inputs - should return cached
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;
        assert_eq!(call_count.load(Ordering::SeqCst), 1); // Still 1, didn't call again

        // Third call with different inputs - should call again
        let different_messages = vec![Message::user("Goodbye".to_string())];
        let _ = client
            .send_message_with_system(&different_messages, &[], Some("System"))
            .await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_caching_disabled() {
        let mock = MockLlmClient::new();
        let call_count = mock.call_count.clone();

        let client = CachingLlmClient::new(
            Box::new(mock),
            "test-model".to_string(),
            CacheConfig::disabled(),
        );

        let messages = vec![Message::user("Hello".to_string())];

        // Both calls should hit the underlying client when caching is disabled
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let mock = MockLlmClient::new();

        let client = CachingLlmClient::new(
            Box::new(mock),
            "test-model".to_string(),
            CacheConfig::default(),
        );

        let messages = vec![Message::user("Hello".to_string())];

        // First call - miss
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;

        // Second call - hit
        let _ = client
            .send_message_with_system(&messages, &[], Some("System"))
            .await;

        let stats = client.cache_stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }
}
