use axum::{
    extract::State,
    response::Json,
};
use serde_json::{json, Value};
use tracing::debug;

use crate::{AppState, Result, AppError};

pub async fn get_cache_stats(
    State(state): State<AppState>,
) -> Result<Json<Value>> {
    debug!("Getting cache statistics");

    let cache_manager = match &state.cache_manager {
        Some(cache) => cache,
        None => {
            return Ok(Json(json!({
                "error": "Cache not configured",
                "stats": null
            })));
        }
    };

    let stats = cache_manager.stats();
    
    Ok(Json(json!({
        "cache_stats": {
            "hits": stats.hits,
            "misses": stats.misses,
            "evictions": stats.evictions,
            "current_size": stats.current_size,
            "max_size": stats.max_size,
            "hit_rate": stats.hit_rate,
            "total_requests": stats.total_requests,
            "hit_rate_percentage": format!("{:.2}%", stats.hit_rate * 100.0)
        },
        "cache_info": {
            "enabled": true,
            "type": "LRU",
            "ttl_seconds": 3600
        }
    })))
}

pub async fn clear_cache(
    State(state): State<AppState>,
) -> Result<Json<Value>> {
    debug!("Clearing cache");

    let cache_manager = match &state.cache_manager {
        Some(cache) => cache,
        None => {
            return Err(AppError::BadRequest("Cache not configured".to_string()));
        }
    };

    let old_size = cache_manager.len();
    cache_manager.clear();
    
    Ok(Json(json!({
        "message": "Cache cleared successfully",
        "entries_removed": old_size
    })))
}

pub async fn invalidate_cache_pattern(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>> {
    debug!("Invalidating cache pattern");

    let cache_manager = match &state.cache_manager {
        Some(cache) => cache,
        None => {
            return Err(AppError::BadRequest("Cache not configured".to_string()));
        }
    };

    let pattern = payload
        .get("pattern")
        .and_then(|p| p.as_str())
        .ok_or_else(|| AppError::BadRequest("Missing 'pattern' field".to_string()))?;

    let old_size = cache_manager.len();
    cache_manager.invalidate_pattern(pattern);
    let new_size = cache_manager.len();
    let removed_count = old_size.saturating_sub(new_size);
    
    Ok(Json(json!({
        "message": "Cache pattern invalidated successfully",
        "pattern": pattern,
        "entries_removed": removed_count
    })))
}

pub async fn get_cache_health(
    State(state): State<AppState>,
) -> Result<Json<Value>> {
    debug!("Getting cache health status");

    let cache_manager = match &state.cache_manager {
        Some(cache) => cache,
        None => {
            return Ok(Json(json!({
                "status": "disabled",
                "healthy": false,
                "message": "Cache not configured"
            })));
        }
    };

    let stats = cache_manager.stats();
    let is_healthy = stats.current_size <= stats.max_size;
    let utilization = if stats.max_size > 0 {
        (stats.current_size as f64 / stats.max_size as f64) * 100.0
    } else {
        0.0
    };
    
    Ok(Json(json!({
        "status": if is_healthy { "healthy" } else { "unhealthy" },
        "healthy": is_healthy,
        "utilization_percentage": format!("{:.2}%", utilization),
        "current_size": stats.current_size,
        "max_size": stats.max_size,
        "performance": {
            "hit_rate": stats.hit_rate,
            "total_requests": stats.total_requests,
            "hits": stats.hits,
            "misses": stats.misses
        }
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache::CacheManager, config::CacheConfig};

    fn create_test_state_with_cache() -> AppState {
        let cache_config = CacheConfig::default();
        let cache_manager = CacheManager::new(cache_config);
        
        AppState::default().with_cache_manager(cache_manager)
    }

    #[tokio::test]
    async fn test_get_cache_stats() {
        let state = create_test_state_with_cache();
        
        let result = get_cache_stats(State(state)).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        let json_value = response.0;
        
        assert!(json_value.get("cache_stats").is_some());
        assert!(json_value.get("cache_info").is_some());
    }

    #[tokio::test]
    async fn test_get_cache_stats_no_cache() {
        let state = AppState::default();
        
        let result = get_cache_stats(State(state)).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        let json_value = response.0;
        
        assert_eq!(json_value.get("error").unwrap().as_str().unwrap(), "Cache not configured");
    }

    #[tokio::test]
    async fn test_clear_cache() {
        let state = create_test_state_with_cache();
        
        if let Some(cache) = &state.cache_manager {
            cache.set("test_key", &"test_value").unwrap();
        }
        
        let result = clear_cache(State(state)).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        let json_value = response.0;
        
        assert_eq!(json_value.get("message").unwrap().as_str().unwrap(), "Cache cleared successfully");
    }

    #[tokio::test]
    async fn test_get_cache_health() {
        let state = create_test_state_with_cache();
        
        let result = get_cache_health(State(state)).await;
        assert!(result.is_ok());
        
        let response = result.unwrap();
        let json_value = response.0;
        
        assert_eq!(json_value.get("status").unwrap().as_str().unwrap(), "healthy");
        assert_eq!(json_value.get("healthy").unwrap().as_bool().unwrap(), true);
    }
}