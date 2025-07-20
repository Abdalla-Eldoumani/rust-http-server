use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use crate::search::{SearchQuery, SearchResult};

#[derive(Debug, Clone)]
pub struct SearchCache {
    cache: Arc<RwLock<HashMap<SearchCacheKey, SearchCacheEntry>>>,
    max_size: usize,
    ttl_seconds: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SearchCacheKey {
    query_hash: u64,
}

#[derive(Debug, Clone)]
struct SearchCacheEntry {
    result: SearchResult,
    created_at: DateTime<Utc>,
    access_count: u64,
    last_accessed: DateTime<Utc>,
}

impl SearchCache {
    pub fn new(max_size: usize, ttl_seconds: i64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            ttl_seconds,
        }
    }

    pub fn get(&self, query: &SearchQuery) -> Option<SearchResult> {
        let key = self.create_cache_key(query);
        let mut cache = self.cache.write();
        
        if let Some(entry) = cache.get_mut(&key) {
            if self.is_entry_valid(&entry) {
                entry.access_count += 1;
                entry.last_accessed = Utc::now();
                return Some(entry.result.clone());
            } else {
                cache.remove(&key);
            }
        }
        
        None
    }

    pub fn put(&self, query: &SearchQuery, result: SearchResult) {
        let key = self.create_cache_key(query);
        let entry = SearchCacheEntry {
            result,
            created_at: Utc::now(),
            access_count: 1,
            last_accessed: Utc::now(),
        };

        let mut cache = self.cache.write();
        
        if cache.len() >= self.max_size && !cache.contains_key(&key) {
            self.evict_lru(&mut cache);
        }
        
        cache.insert(key, entry);
    }

    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }

    pub fn invalidate_by_pattern(&self, pattern: &str) {
        let mut cache = self.cache.write();
        if !pattern.is_empty() {
            cache.clear();
        }
    }

    pub fn get_stats(&self) -> SearchCacheStats {
        let cache = self.cache.read();
        let _now = Utc::now();
        
        let mut total_entries = 0;
        let mut expired_entries = 0;
        let mut total_access_count = 0;
        
        for entry in cache.values() {
            total_entries += 1;
            total_access_count += entry.access_count;
            
            if !self.is_entry_valid(entry) {
                expired_entries += 1;
            }
        }

        SearchCacheStats {
            total_entries,
            expired_entries,
            max_size: self.max_size,
            ttl_seconds: self.ttl_seconds,
            total_access_count,
            hit_rate: 0.0,
        }
    }

    pub fn cleanup_expired(&self) {
        let mut cache = self.cache.write();
        let keys_to_remove: Vec<SearchCacheKey> = cache
            .iter()
            .filter(|(_, entry)| !self.is_entry_valid(entry))
            .map(|(key, _)| key.clone())
            .collect();

        for key in keys_to_remove {
            cache.remove(&key);
        }
    }

    fn create_cache_key(&self, query: &SearchQuery) -> SearchCacheKey {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        
        query.text.hash(&mut hasher);
        query.tags.hash(&mut hasher);
        query.created_date_range.as_ref().map(|dr| {
            dr.start.hash(&mut hasher);
            dr.end.hash(&mut hasher);
        });
        query.updated_date_range.as_ref().map(|dr| {
            dr.start.hash(&mut hasher);
            dr.end.hash(&mut hasher);
        });
        query.sort_criteria.len().hash(&mut hasher);
        for criterion in &query.sort_criteria {
            std::mem::discriminant(&criterion.field).hash(&mut hasher);
            std::mem::discriminant(&criterion.order).hash(&mut hasher);
        }
        query.offset.hash(&mut hasher);
        query.limit.hash(&mut hasher);
        query.fuzzy.hash(&mut hasher);
        query.created_by.hash(&mut hasher);
        query.min_relevance.map(|r| (r * 1000.0) as i64).hash(&mut hasher);

        SearchCacheKey {
            query_hash: hasher.finish(),
        }
    }

    fn is_entry_valid(&self, entry: &SearchCacheEntry) -> bool {
        let now = Utc::now();
        let age = now.signed_duration_since(entry.created_at);
        age.num_seconds() < self.ttl_seconds
    }

    fn evict_lru(&self, cache: &mut HashMap<SearchCacheKey, SearchCacheEntry>) {
        if let Some((key_to_remove, _)) = cache
            .iter()
            .min_by_key(|(_, entry)| entry.last_accessed)
            .map(|(k, v)| (k.clone(), v.clone()))
        {
            cache.remove(&key_to_remove);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub max_size: usize,
    pub ttl_seconds: i64,
    pub total_access_count: u64,
    pub hit_rate: f64,
}

impl Default for SearchCache {
    fn default() -> Self {
        Self::new(1000, 300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{SearchQuery, SearchResult};

    #[test]
    fn test_cache_basic_operations() {
        let cache = SearchCache::new(10, 300);
        let query = SearchQuery::new().with_text("test".to_string());
        let result = SearchResult {
            items: vec![],
            total_count: 0,
            offset: 0,
            limit: 10,
            has_more: false,
        };

        assert!(cache.get(&query).is_none());

        cache.put(&query, result.clone());
        assert!(cache.get(&query).is_some());

        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.expired_entries, 0);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = SearchCache::new(2, 300);
        
        let query1 = SearchQuery::new().with_text("test1".to_string());
        let query2 = SearchQuery::new().with_text("test2".to_string());
        let query3 = SearchQuery::new().with_text("test3".to_string());
        
        let result = SearchResult {
            items: vec![],
            total_count: 0,
            offset: 0,
            limit: 10,
            has_more: false,
        };

        cache.put(&query1, result.clone());
        cache.put(&query2, result.clone());
        
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 2);

        cache.put(&query3, result.clone());
        let stats = cache.get_stats();
        assert_eq!(stats.total_entries, 2);
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = SearchCache::new(10, 300);
        let query = SearchQuery::new().with_text("test".to_string());
        let result = SearchResult {
            items: vec![],
            total_count: 0,
            offset: 0,
            limit: 10,
            has_more: false,
        };

        cache.put(&query, result);
        assert!(cache.get(&query).is_some());

        cache.invalidate_all();
        assert!(cache.get(&query).is_none());
    }
}