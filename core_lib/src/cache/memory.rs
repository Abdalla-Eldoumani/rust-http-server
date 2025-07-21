use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{debug, warn};
use crate::config::CacheConfig;

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub data: serde_json::Value,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub access_count: u64,
}

impl CacheEntry {
    pub fn new(data: serde_json::Value, ttl: Option<Duration>) -> Self {
        let now = Utc::now();
        let expires_at = ttl.map(|duration| {
            now + chrono::Duration::from_std(duration).unwrap_or(chrono::Duration::seconds(300))
        });

        Self {
            data,
            expires_at,
            created_at: now,
            access_count: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at
        } else {
            false
        }
    }

    pub fn increment_access(&mut self) {
        self.access_count += 1;
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub current_size: usize,
    pub max_size: usize,
    pub hit_rate: f64,
    pub total_requests: u64,
}

impl CacheStats {
    pub fn new(max_size: usize) -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            current_size: 0,
            max_size,
            hit_rate: 0.0,
            total_requests: 0,
        }
    }

    pub fn record_hit(&mut self) {
        self.hits += 1;
        self.total_requests += 1;
        self.update_hit_rate();
    }

    pub fn record_miss(&mut self) {
        self.misses += 1;
        self.total_requests += 1;
        self.update_hit_rate();
    }

    pub fn record_eviction(&mut self) {
        self.evictions += 1;
    }

    pub fn update_size(&mut self, size: usize) {
        self.current_size = size;
    }

    fn update_hit_rate(&mut self) {
        if self.total_requests > 0 {
            self.hit_rate = self.hits as f64 / self.total_requests as f64;
        }
    }
}

#[derive(Debug)]
pub struct CacheManager {
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    config: CacheConfig,
    stats: Arc<RwLock<CacheStats>>,
    last_cleanup: Arc<RwLock<Instant>>,
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            cache: Arc::clone(&self.cache),
            config: self.config.clone(),
            stats: Arc::clone(&self.stats),
            last_cleanup: Arc::clone(&self.last_cleanup),
        }
    }
}

impl CacheManager {
    pub fn new(config: CacheConfig) -> Self {
        let cache = Arc::new(RwLock::new(LruCache::new(
            std::num::NonZeroUsize::new(config.max_size).unwrap_or(std::num::NonZeroUsize::new(1000).unwrap())
        )));
        let stats = Arc::new(RwLock::new(CacheStats::new(config.max_size)));
        let last_cleanup = Arc::new(RwLock::new(Instant::now()));

        Self {
            cache,
            config,
            stats,
            last_cleanup,
        }
    }

    pub fn default() -> Self {
        Self::new(CacheConfig::default())
    }

    pub fn generate_key(&self, prefix: &str, components: &[&str]) -> String {
        let mut key = prefix.to_string();
        for component in components {
            key.push(':');
            key.push_str(component);
        }
        key
    }

    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.cleanup_expired_if_needed();

        let mut cache = self.cache.write();
        
        if let Some(entry) = cache.get_mut(key) {
            if entry.is_expired() {
                cache.pop(key);
                if self.config.enable_stats {
                    self.stats.write().record_miss();
                }
                debug!("Cache entry expired for key: {}", key);
                return None;
            }

            entry.increment_access();
            
            if self.config.enable_stats {
                self.stats.write().record_hit();
            }

            match serde_json::from_value(entry.data.clone()) {
                Ok(value) => {
                    debug!("Cache hit for key: {}", key);
                    Some(value)
                }
                Err(e) => {
                    warn!("Failed to deserialize cached value for key {}: {}", key, e);
                    cache.pop(key);
                    if self.config.enable_stats {
                        self.stats.write().record_miss();
                    }
                    None
                }
            }
        } else {
            if self.config.enable_stats {
                self.stats.write().record_miss();
            }
            debug!("Cache miss for key: {}", key);
            None
        }
    }

    pub fn set<T>(&self, key: &str, value: &T) -> Result<(), serde_json::Error>
    where
        T: Serialize,
    {
        let ttl = Some(Duration::from_secs(self.config.default_ttl_seconds));
        self.set_with_ttl(key, value, ttl)
    }

    pub fn set_with_ttl<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), serde_json::Error>
    where
        T: Serialize,
    {
        let data = serde_json::to_value(value)?;
        let entry = CacheEntry::new(data, ttl);

        let mut cache = self.cache.write();
        let was_evicted = cache.put(key.to_string(), entry).is_some();
        
        if was_evicted && self.config.enable_stats {
            self.stats.write().record_eviction();
        }

        if self.config.enable_stats {
            self.stats.write().update_size(cache.len());
        }

        debug!("Cached value for key: {} (TTL: {:?})", key, ttl);
        Ok(())
    }

    pub fn remove(&self, key: &str) -> bool {
        let mut cache = self.cache.write();
        let removed = cache.pop(key).is_some();
        
        if self.config.enable_stats {
            self.stats.write().update_size(cache.len());
        }

        if removed {
            debug!("Removed cache entry for key: {}", key);
        }
        
        removed
    }

    pub fn clear(&self) {
        let mut cache = self.cache.write();
        cache.clear();
        
        if self.config.enable_stats {
            self.stats.write().update_size(0);
        }
        
        debug!("Cleared all cache entries");
    }

    pub fn invalidate_pattern(&self, pattern: &str) {
        let mut cache = self.cache.write();
        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, _)| {
                if key.contains(pattern) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        let removed_count = keys_to_remove.len();
        for key in keys_to_remove {
            cache.pop(&key);
        }

        if self.config.enable_stats {
            self.stats.write().update_size(cache.len());
        }

        debug!("Invalidated {} cache entries matching pattern: {}", removed_count, pattern);
    }

    pub fn stats(&self) -> CacheStats {
        if self.config.enable_stats {
            let mut stats = self.stats.write();
            stats.update_size(self.cache.read().len());
            stats.clone()
        } else {
            CacheStats::new(self.config.max_size)
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.cache.read().contains(key)
    }

    pub fn len(&self) -> usize {
        self.cache.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }

    fn cleanup_expired_if_needed(&self) {
        const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
        
        let now = Instant::now();
        let mut last_cleanup = self.last_cleanup.write();
        
        if now.duration_since(*last_cleanup) > CLEANUP_INTERVAL {
            *last_cleanup = now;
            drop(last_cleanup);
            self.cleanup_expired();
        }
    }

    fn cleanup_expired(&self) {
        let mut cache = self.cache.write();
        let mut expired_keys = Vec::new();

        for (key, entry) in cache.iter() {
            if entry.is_expired() {
                expired_keys.push(key.clone());
            }
        }

        for key in expired_keys {
            cache.pop(&key);
        }

        if self.config.enable_stats {
            self.stats.write().update_size(cache.len());
        }

        debug!("Cleaned up expired cache entries");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_cache_basic_operations() {
        let cache = CacheManager::default();
        
        cache.set("test_key", &"test_value").unwrap();
        let value: Option<String> = cache.get("test_key");
        assert_eq!(value, Some("test_value".to_string()));
        
        let missing: Option<String> = cache.get("missing_key");
        assert_eq!(missing, None);
        
        assert!(cache.remove("test_key"));
        let removed: Option<String> = cache.get("test_key");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_cache_ttl() {
        let config = CacheConfig {
            max_size: 100,
            default_ttl_seconds: 1,
            cleanup_interval_seconds: 60,
            enable_stats: true,
        };
        let cache = CacheManager::new(config);
        
        cache.set("ttl_key", &"ttl_value").unwrap();
        let value: Option<String> = cache.get("ttl_key");
        assert_eq!(value, Some("ttl_value".to_string()));
        
        thread::sleep(Duration::from_secs(2));
        
        let expired: Option<String> = cache.get("ttl_key");
        assert_eq!(expired, None);
    }

    #[test]
    fn test_cache_key_generation() {
        let cache = CacheManager::default();
        
        let key = cache.generate_key("items", &["user", "123", "active"]);
        assert_eq!(key, "items:user:123:active");
    }

    #[test]
    fn test_cache_pattern_invalidation() {
        let cache = CacheManager::default();
        
        cache.set("user:123:profile", &"profile_data").unwrap();
        cache.set("user:123:settings", &"settings_data").unwrap();
        cache.set("user:456:profile", &"other_profile").unwrap();
        
        cache.invalidate_pattern("user:123");
        
        let profile: Option<String> = cache.get("user:123:profile");
        let settings: Option<String> = cache.get("user:123:settings");
        let other: Option<String> = cache.get("user:456:profile");
        
        assert_eq!(profile, None);
        assert_eq!(settings, None);
        assert_eq!(other, Some("other_profile".to_string()));
    }

    #[test]
    fn test_cache_stats() {
        let cache = CacheManager::default();
        
        cache.set("key1", &"value1").unwrap();
        cache.set("key2", &"value2").unwrap();
        
        let _: Option<String> = cache.get("key1");
        let _: Option<String> = cache.get("key1");
        let _: Option<String> = cache.get("missing");
        
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.total_requests, 3);
        assert!((stats.hit_rate - 0.6666666666666666).abs() < 0.0001);
    }
}