//! Cache storage implementations for LLM response caching

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::llm::{LlmResponse, Message, ToolDefinition};

/// Cache key for identifying unique LLM requests
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    /// Hash of the system prompt
    pub system_prompt_hash: u64,
    /// Hash of the message history
    pub messages_hash: u64,
    /// Hash of the tool definitions
    pub tools_hash: u64,
    /// Model identifier
    pub model: String,
}

impl CacheKey {
    /// Generate a cache key from request components
    pub fn from_request(
        system_prompt: Option<&str>,
        messages: &[Message],
        tools: &[ToolDefinition],
        model: &str,
    ) -> Self {
        use std::collections::hash_map::DefaultHasher;

        let mut system_hasher = DefaultHasher::new();
        if let Some(prompt) = system_prompt {
            prompt.hash(&mut system_hasher);
        }

        let mut messages_hasher = DefaultHasher::new();
        // Hash message content - we use JSON serialization for consistency
        if let Ok(json) = serde_json::to_string(messages) {
            json.hash(&mut messages_hasher);
        }

        let mut tools_hasher = DefaultHasher::new();
        if let Ok(json) = serde_json::to_string(tools) {
            json.hash(&mut tools_hasher);
        }

        Self {
            system_prompt_hash: system_hasher.finish(),
            messages_hash: messages_hasher.finish(),
            tools_hash: tools_hasher.finish(),
            model: model.to_string(),
        }
    }

    /// Create a unique string key for HashMap storage
    fn to_string_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.model, self.system_prompt_hash, self.messages_hash, self.tools_hash
        )
    }
}

impl Hash for CacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.system_prompt_hash.hash(state);
        self.messages_hash.hash(state);
        self.tools_hash.hash(state);
        self.model.hash(state);
    }
}

/// Cached LLM response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    /// The cached response
    pub response: LlmResponse,
    /// When this entry was cached
    #[serde(skip)]
    pub cached_at: Option<Instant>,
    /// Time-to-live for this entry
    #[serde(skip)]
    pub ttl: Option<Duration>,
}

impl CachedResponse {
    /// Create a new cached response
    pub fn new(response: LlmResponse, ttl: Duration) -> Self {
        Self {
            response,
            cached_at: Some(Instant::now()),
            ttl: Some(ttl),
        }
    }

    /// Check if this cache entry has expired
    pub fn is_expired(&self) -> bool {
        match (self.cached_at, self.ttl) {
            (Some(cached_at), Some(ttl)) => cached_at.elapsed() > ttl,
            _ => false,
        }
    }
}

/// Statistics about cache usage
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Total tokens saved by cache hits
    pub tokens_saved: usize,
    /// Estimated cost saved (in dollars)
    pub estimated_cost_saved: f64,
    /// Current number of entries in cache
    pub entries: usize,
    /// Total size of cached data (approximate bytes)
    pub size_bytes: usize,
}

impl CacheStats {
    /// Calculate hit rate as a percentage
    pub fn hit_rate(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f32 / total as f32) * 100.0
        }
    }

    /// Record a cache hit
    pub fn record_hit(&mut self, tokens_saved: usize) {
        self.hits += 1;
        self.tokens_saved += tokens_saved;
        // Estimate cost savings: ~$3/1M tokens for input
        self.estimated_cost_saved += (tokens_saved as f64 / 1_000_000.0) * 3.0;
    }

    /// Record a cache miss
    pub fn record_miss(&mut self) {
        self.misses += 1;
    }
}

/// Trait for cache storage backends
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Get a cached response by key
    async fn get(&self, key: &CacheKey) -> Option<CachedResponse>;

    /// Store a response in the cache
    async fn set(&self, key: &CacheKey, response: CachedResponse);

    /// Invalidate a specific cache entry
    async fn invalidate(&self, key: &CacheKey);

    /// Clear all cache entries
    async fn clear(&self);

    /// Get cache statistics
    async fn stats(&self) -> CacheStats;
}

/// In-memory LRU cache implementation
pub struct MemoryCache {
    /// Cache entries
    entries: Arc<RwLock<HashMap<String, CachedResponse>>>,
    /// Maximum number of entries
    max_entries: usize,
    /// Default TTL for entries
    default_ttl: Duration,
    /// Statistics
    stats: Arc<RwLock<CacheStats>>,
    /// Access order for LRU eviction (key -> last access time)
    access_order: Arc<RwLock<HashMap<String, Instant>>>,
}

impl MemoryCache {
    /// Create a new memory cache with specified limits
    pub fn new(max_entries: usize, default_ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
            default_ttl,
            stats: Arc::new(RwLock::new(CacheStats::default())),
            access_order: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a memory cache with default settings (100 entries, 30 min TTL)
    pub fn default_config() -> Self {
        Self::new(100, Duration::from_secs(30 * 60))
    }

    /// Evict expired entries and enforce LRU limit
    async fn evict_if_needed(&self) {
        let mut entries = self.entries.write().await;
        let mut access_order = self.access_order.write().await;

        // Remove expired entries
        let expired_keys: Vec<String> = entries
            .iter()
            .filter(|(_, v)| v.is_expired())
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired_keys {
            entries.remove(&key);
            access_order.remove(&key);
        }

        // If still over limit, evict LRU entries
        while entries.len() >= self.max_entries {
            if let Some((oldest_key, _)) = access_order
                .iter()
                .min_by_key(|(_, &time)| time)
                .map(|(k, t)| (k.clone(), *t))
            {
                entries.remove(&oldest_key);
                access_order.remove(&oldest_key);
            } else {
                break;
            }
        }

        // Update stats
        let mut stats = self.stats.write().await;
        stats.entries = entries.len();
    }
}

#[async_trait]
impl CacheStore for MemoryCache {
    async fn get(&self, key: &CacheKey) -> Option<CachedResponse> {
        let string_key = key.to_string_key();
        let entries = self.entries.read().await;

        if let Some(entry) = entries.get(&string_key) {
            if entry.is_expired() {
                // Entry expired, will be cleaned up later
                let mut stats = self.stats.write().await;
                stats.record_miss();
                return None;
            }

            // Update access time
            {
                let mut access_order = self.access_order.write().await;
                access_order.insert(string_key.clone(), Instant::now());
            }

            // Record hit
            let tokens = entry
                .response
                .usage
                .as_ref()
                .map(|u| u.input_tokens)
                .unwrap_or(0);
            {
                let mut stats = self.stats.write().await;
                stats.record_hit(tokens);
            }

            Some(entry.clone())
        } else {
            let mut stats = self.stats.write().await;
            stats.record_miss();
            None
        }
    }

    async fn set(&self, key: &CacheKey, response: CachedResponse) {
        self.evict_if_needed().await;

        let string_key = key.to_string_key();
        let mut entries = self.entries.write().await;
        let mut access_order = self.access_order.write().await;

        entries.insert(string_key.clone(), response);
        access_order.insert(string_key, Instant::now());

        // Update stats
        let mut stats = self.stats.write().await;
        stats.entries = entries.len();
    }

    async fn invalidate(&self, key: &CacheKey) {
        let string_key = key.to_string_key();
        let mut entries = self.entries.write().await;
        let mut access_order = self.access_order.write().await;

        entries.remove(&string_key);
        access_order.remove(&string_key);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.entries = entries.len();
    }

    async fn clear(&self) {
        let mut entries = self.entries.write().await;
        let mut access_order = self.access_order.write().await;
        let mut stats = self.stats.write().await;

        entries.clear();
        access_order.clear();
        stats.entries = 0;
    }

    async fn stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ContentBlock, Message, Role, TokenUsage};

    fn create_test_response() -> LlmResponse {
        LlmResponse {
            message: Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text {
                    text: "Test response".to_string(),
                }],
            },
            usage: Some(TokenUsage::new(100, 50)),
        }
    }

    #[tokio::test]
    async fn test_cache_key_generation() {
        let key1 = CacheKey::from_request(
            Some("System prompt"),
            &[Message::user("Hello".to_string())],
            &[],
            "test-model",
        );

        let key2 = CacheKey::from_request(
            Some("System prompt"),
            &[Message::user("Hello".to_string())],
            &[],
            "test-model",
        );

        // Same inputs should produce same key
        assert_eq!(key1, key2);

        let key3 = CacheKey::from_request(
            Some("Different prompt"),
            &[Message::user("Hello".to_string())],
            &[],
            "test-model",
        );

        // Different inputs should produce different key
        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_memory_cache_basic() {
        let cache = MemoryCache::new(10, Duration::from_secs(60));

        let key = CacheKey::from_request(Some("test"), &[], &[], "model");
        let response = CachedResponse::new(create_test_response(), Duration::from_secs(60));

        // Cache miss
        assert!(cache.get(&key).await.is_none());

        // Set and get
        cache.set(&key, response.clone()).await;
        let cached = cache.get(&key).await;
        assert!(cached.is_some());

        // Stats should reflect hit
        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let cache = MemoryCache::new(10, Duration::from_millis(10));

        let key = CacheKey::from_request(Some("test"), &[], &[], "model");
        let response = CachedResponse::new(create_test_response(), Duration::from_millis(10));

        cache.set(&key, response).await;

        // Should be present immediately
        assert!(cache.get(&key).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Should be expired now
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_lru_eviction() {
        let cache = MemoryCache::new(2, Duration::from_secs(60));

        let key1 = CacheKey::from_request(Some("test1"), &[], &[], "model");
        let key2 = CacheKey::from_request(Some("test2"), &[], &[], "model");
        let key3 = CacheKey::from_request(Some("test3"), &[], &[], "model");

        let response = CachedResponse::new(create_test_response(), Duration::from_secs(60));

        cache.set(&key1, response.clone()).await;
        cache.set(&key2, response.clone()).await;

        // Access key1 to make it more recent
        cache.get(&key1).await;

        // Add key3, should evict key2 (least recently used)
        cache.set(&key3, response).await;

        // key1 and key3 should be present
        assert!(cache.get(&key1).await.is_some());
        assert!(cache.get(&key3).await.is_some());
    }
}
