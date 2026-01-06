//! Token caching module for reducing LLM costs
//!
//! This module provides a provider-agnostic caching layer that works with
//! any LLM client. It supports both in-memory caching and tracks cache
//! statistics for cost analysis.

mod store;

pub use store::{CacheKey, CacheStats, CacheStore, CachedResponse, MemoryCache};
