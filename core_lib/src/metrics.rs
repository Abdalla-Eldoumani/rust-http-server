//! Server metrics collection and reporting

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Clone)]
pub struct MetricsCollector {
    pub total_requests: Arc<AtomicU64>,
    pub successful_requests: Arc<AtomicU64>,
    pub failed_requests: Arc<AtomicU64>,
    pub requests_by_method: Arc<RwLock<HashMap<String, u64>>>,
    pub requests_by_endpoint: Arc<RwLock<HashMap<String, u64>>>,
    pub response_times: Arc<RwLock<Vec<ResponseTime>>>,
    pub start_time: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ResponseTime {
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u128,
    pub endpoint: String,
    pub status: u16,
}

#[derive(Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub requests_by_method: HashMap<String, u64>,
    pub requests_by_endpoint: Vec<EndpointMetric>,
    pub average_response_time_ms: f64,
    pub uptime_seconds: i64,
    pub requests_per_second: f64,
    pub error_rate: f64,
    pub last_hour_response_times: Vec<ResponseTime>,
}

#[derive(Serialize, Deserialize)]
pub struct EndpointMetric {
    pub endpoint: String,
    pub count: u64,
    pub percentage: f64,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            total_requests: Arc::new(AtomicU64::new(0)),
            successful_requests: Arc::new(AtomicU64::new(0)),
            failed_requests: Arc::new(AtomicU64::new(0)),
            requests_by_method: Arc::new(RwLock::new(HashMap::new())),
            requests_by_endpoint: Arc::new(RwLock::new(HashMap::new())),
            response_times: Arc::new(RwLock::new(Vec::new())),
            start_time: Utc::now(),
        }
    }

    pub fn record_request(&self, method: &str, endpoint: &str) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        
        let mut methods = self.requests_by_method.write();
        *methods.entry(method.to_string()).or_insert(0) += 1;
        
        let mut endpoints = self.requests_by_endpoint.write();
        *endpoints.entry(endpoint.to_string()).or_insert(0) += 1;
    }

    pub fn record_response(&self, endpoint: &str, duration_ms: u128, status: u16) {
        if status < 400 {
            self.successful_requests.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_requests.fetch_add(1, Ordering::Relaxed);
        }

        let response_time = ResponseTime {
            timestamp: Utc::now(),
            duration_ms,
            endpoint: endpoint.to_string(),
            status,
        };

        let mut times = self.response_times.write();
        times.push(response_time);
        
        if times.len() > 1000 {
            let drain_end = times.len() - 1000;
            times.drain(0..drain_end);
        }
    }

    pub fn get_snapshot(&self, _item_count: usize) -> MetricsSnapshot {
        let total = self.total_requests.load(Ordering::Relaxed);
        let successful = self.successful_requests.load(Ordering::Relaxed);
        let failed = self.failed_requests.load(Ordering::Relaxed);
        
        let uptime = Utc::now().signed_duration_since(self.start_time);
        let uptime_seconds = uptime.num_seconds().max(1);
        
        let methods = self.requests_by_method.read().clone();
        let endpoints = self.requests_by_endpoint.read();
        
        let mut endpoint_metrics: Vec<EndpointMetric> = endpoints
            .iter()
            .map(|(endpoint, count)| EndpointMetric {
                endpoint: endpoint.clone(),
                count: *count,
                percentage: if total > 0 {
                    (*count as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
            })
            .collect();
        
        endpoint_metrics.sort_by(|a, b| b.count.cmp(&a.count));
        
        let one_hour_ago = Utc::now() - chrono::Duration::hours(1);
        let times = self.response_times.read();
        let recent_times: Vec<ResponseTime> = times
            .iter()
            .filter(|t| t.timestamp > one_hour_ago)
            .cloned()
            .collect();
        
        let avg_response_time = if !recent_times.is_empty() {
            recent_times.iter().map(|t| t.duration_ms as f64).sum::<f64>() / recent_times.len() as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            requests_by_method: methods,
            requests_by_endpoint: endpoint_metrics,
            average_response_time_ms: avg_response_time,
            uptime_seconds,
            requests_per_second: total as f64 / uptime_seconds as f64,
            error_rate: if total > 0 {
                (failed as f64 / total as f64) * 100.0
            } else {
                0.0
            },
            last_hour_response_times: recent_times,
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}