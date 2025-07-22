//! System resource monitoring and metrics collection

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use sysinfo::{System, Networks, Pid};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_usage_percent: f32,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
    pub memory_usage_percent: f64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_usage_percent: f64,
    pub load_average: Option<(f64, f64, f64)>,
    pub process_count: usize,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub name: String,
    pub mount_point: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub usage_percent: f64,
    pub file_system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStats {
    pub interface: String,
    pub bytes_received: u64,
    pub bytes_transmitted: u64,
    pub packets_received: u64,
    pub packets_transmitted: u64,
    pub errors_received: u64,
    pub errors_transmitted: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub status: String,
    pub start_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub resource_usage: ResourceUsage,
    pub disk_usage: Vec<DiskUsage>,
    pub network_stats: Vec<NetworkStats>,
    pub current_process: Option<ProcessInfo>,
    pub system_info: SystemInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: Option<String>,
    pub kernel_version: Option<String>,
    pub os_version: Option<String>,
    pub architecture: String,
    pub cpu_count: usize,
    pub cpu_brand: String,
    pub cpu_frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub response_time_p50: f64,
    pub response_time_p95: f64,
    pub response_time_p99: f64,
    pub requests_per_second: f64,
    pub error_rate_percent: f64,
    pub active_connections: usize,
    pub memory_usage_trend: Vec<f64>,
    pub cpu_usage_trend: Vec<f32>,
}

pub struct SystemMonitor {
    system: Arc<Mutex<System>>,
    start_time: Instant,
    current_pid: Option<u32>,
    metrics_history: Arc<Mutex<Vec<SystemMetrics>>>,
    max_history_size: usize,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        
        let current_pid = std::process::id();
        
        Self {
            system: Arc::new(Mutex::new(system)),
            start_time: Instant::now(),
            current_pid: Some(current_pid),
            metrics_history: Arc::new(Mutex::new(Vec::new())),
            max_history_size: 100,
        }
    }

    pub fn collect_metrics(&self) -> SystemMetrics {
        let mut system = self.system.lock().unwrap();
        
        system.refresh_all();
        
        let timestamp = chrono::Utc::now();
        
        let resource_usage = self.collect_resource_usage(&system);
        
        let disk_usage = self.collect_disk_usage(&system);
        
        let network_stats = self.collect_network_stats(&system);
        
        let current_process = self.collect_current_process_info(&system);
        
        let system_info = self.collect_system_info(&system);
        
        let metrics = SystemMetrics {
            timestamp,
            resource_usage,
            disk_usage,
            network_stats,
            current_process,
            system_info,
        };
        
        self.store_metrics_history(metrics.clone());
        
        debug!("Collected system metrics: CPU: {:.1}%, Memory: {:.1}%", metrics.resource_usage.cpu_usage_percent, metrics.resource_usage.memory_usage_percent);
        
        metrics
    }

    fn collect_resource_usage(&self, system: &System) -> ResourceUsage {
        let cpu_usage = system.global_cpu_info().cpu_usage();
        
        let memory_total = system.total_memory();
        let memory_used = system.used_memory();
        let memory_usage_percent = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };
        
        let swap_total = system.total_swap();
        let swap_used = system.used_swap();
        let swap_usage_percent = if swap_total > 0 {
            (swap_used as f64 / swap_total as f64) * 100.0
        } else {
            0.0
        };
        
        let load_average = System::load_average();
        let load_avg = if load_average.one > 0.0 || load_average.five > 0.0 || load_average.fifteen > 0.0 {
            Some((load_average.one, load_average.five, load_average.fifteen))
        } else {
            None
        };
        
        let process_count = system.processes().len();
        let uptime_seconds = System::uptime();
        
        ResourceUsage {
            cpu_usage_percent: cpu_usage,
            memory_used_bytes: memory_used,
            memory_total_bytes: memory_total,
            memory_usage_percent,
            swap_used_bytes: swap_used,
            swap_total_bytes: swap_total,
            swap_usage_percent,
            load_average: load_avg,
            process_count,
            uptime_seconds,
        }
    }

    fn collect_disk_usage(&self, _system: &System) -> Vec<DiskUsage> {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        disks.iter().map(|disk| {
            let total_bytes = disk.total_space();
            let available_bytes = disk.available_space();
            let used_bytes = total_bytes - available_bytes;
            let usage_percent = if total_bytes > 0 {
                (used_bytes as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };
            
            DiskUsage {
                name: disk.name().to_string_lossy().to_string(),
                mount_point: disk.mount_point().to_string_lossy().to_string(),
                total_bytes,
                available_bytes,
                used_bytes,
                usage_percent,
                file_system: disk.file_system().to_string_lossy().to_string(),
            }
        }).collect()
    }

    fn collect_network_stats(&self, _system: &System) -> Vec<NetworkStats> {
        let networks = Networks::new_with_refreshed_list();
        networks.iter().map(|(interface_name, network)| {
            NetworkStats {
                interface: interface_name.clone(),
                bytes_received: network.received(),
                bytes_transmitted: network.transmitted(),
                packets_received: network.packets_received(),
                packets_transmitted: network.packets_transmitted(),
                errors_received: network.errors_on_received(),
                errors_transmitted: network.errors_on_transmitted(),
            }
        }).collect()
    }

    fn collect_current_process_info(&self, system: &System) -> Option<ProcessInfo> {
        if let Some(pid) = self.current_pid {
            if let Some(process) = system.process(Pid::from(pid as usize)) {
                return Some(ProcessInfo {
                    pid,
                    name: process.name().to_string(),
                    cpu_usage: process.cpu_usage(),
                    memory_bytes: process.memory(),
                    virtual_memory_bytes: process.virtual_memory(),
                    status: format!("{:?}", process.status()),
                    start_time: process.start_time(),
                });
            }
        }
        None
    }

    fn collect_system_info(&self, system: &System) -> SystemInfo {
        let hostname = System::host_name();
        let kernel_version = System::kernel_version();
        let os_version = System::long_os_version();
        
        let cpu_count = system.cpus().len();
        let cpu_brand = system.cpus().first()
            .map(|cpu| cpu.brand().to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        let cpu_frequency = system.cpus().first()
            .map(|cpu| cpu.frequency())
            .unwrap_or(0);
        
        SystemInfo {
            hostname,
            kernel_version,
            os_version,
            architecture: std::env::consts::ARCH.to_string(),
            cpu_count,
            cpu_brand,
            cpu_frequency_mhz: cpu_frequency,
        }
    }

    fn store_metrics_history(&self, metrics: SystemMetrics) {
        let mut history = self.metrics_history.lock().unwrap();
        history.push(metrics);
        
        if history.len() > self.max_history_size {
            let excess = history.len() - self.max_history_size;
            history.drain(0..excess);
        }
    }

    pub fn get_metrics_history(&self) -> Vec<SystemMetrics> {
        self.metrics_history.lock().unwrap().clone()
    }

    pub fn get_performance_metrics(&self, app_metrics: &crate::metrics::MetricsSnapshot) -> PerformanceMetrics {
        let history = self.get_metrics_history();
        
        let mut response_times: Vec<f64> = app_metrics.last_hour_response_times
            .iter()
            .map(|rt| rt.duration_ms as f64)
            .collect();
        response_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let p50 = percentile(&response_times, 50.0);
        let p95 = percentile(&response_times, 95.0);
        let p99 = percentile(&response_times, 99.0);
        
        let recent_count = 10.min(history.len());
        let recent_metrics = if history.len() >= recent_count {
            &history[history.len() - recent_count..]
        } else {
            &history[..]
        };
        
        let memory_usage_trend: Vec<f64> = recent_metrics
            .iter()
            .map(|m| m.resource_usage.memory_usage_percent)
            .collect();
        
        let cpu_usage_trend: Vec<f32> = recent_metrics
            .iter()
            .map(|m| m.resource_usage.cpu_usage_percent)
            .collect();
        
        PerformanceMetrics {
            response_time_p50: p50,
            response_time_p95: p95,
            response_time_p99: p99,
            requests_per_second: app_metrics.requests_per_second,
            error_rate_percent: app_metrics.error_rate,
            active_connections: 0,
            memory_usage_trend,
            cpu_usage_trend,
        }
    }

    pub fn check_resource_alerts(&self, metrics: &SystemMetrics) -> Vec<String> {
        let mut alerts = Vec::new();
        
        if metrics.resource_usage.cpu_usage_percent > 90.0 {
            alerts.push(format!("High CPU usage: {:.1}%", metrics.resource_usage.cpu_usage_percent));
        } else if metrics.resource_usage.cpu_usage_percent > 80.0 {
            alerts.push(format!("Elevated CPU usage: {:.1}%", metrics.resource_usage.cpu_usage_percent));
        }
        
        if metrics.resource_usage.memory_usage_percent > 90.0 {
            alerts.push(format!("High memory usage: {:.1}%", metrics.resource_usage.memory_usage_percent));
        } else if metrics.resource_usage.memory_usage_percent > 80.0 {
            alerts.push(format!("Elevated memory usage: {:.1}%", metrics.resource_usage.memory_usage_percent));
        }
        
        for disk in &metrics.disk_usage {
            if disk.usage_percent > 95.0 {
                alerts.push(format!("Critical disk usage on {}: {:.1}%", disk.mount_point, disk.usage_percent));
            } else if disk.usage_percent > 85.0 {
                alerts.push(format!("High disk usage on {}: {:.1}%", disk.mount_point, disk.usage_percent));
            }
        }
        
        if let Some(process) = &metrics.current_process {
            if process.memory_bytes > 1024 * 1024 * 1024 { // 1GB
                alerts.push(format!("High process memory usage: {:.1} MB", 
                    process.memory_bytes as f64 / (1024.0 * 1024.0)));
            }
        }
        
        alerts
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn percentile(sorted_values: &[f64], percentile: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    
    if percentile <= 0.0 {
        return sorted_values[0];
    }
    
    if percentile >= 100.0 {
        return sorted_values[sorted_values.len() - 1];
    }
    
    let index = (percentile / 100.0) * (sorted_values.len() - 1) as f64;
    let lower_index = index.floor() as usize;
    let upper_index = index.ceil() as usize;
    
    if lower_index == upper_index {
        sorted_values[lower_index]
    } else {
        let weight = index - lower_index as f64;
        sorted_values[lower_index] * (1.0 - weight) + sorted_values[upper_index] * weight
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_monitor_creation() {
        let monitor = SystemMonitor::new();
        assert!(monitor.current_pid.is_some());
    }

    #[test]
    fn test_metrics_collection() {
        let monitor = SystemMonitor::new();
        let metrics = monitor.collect_metrics();
        
        assert!(metrics.resource_usage.cpu_usage_percent >= 0.0);
        assert!(metrics.resource_usage.memory_total_bytes > 0);
        assert!(!metrics.disk_usage.is_empty());
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&values, 50.0), 3.0);
        assert_eq!(percentile(&values, 0.0), 1.0);
        assert_eq!(percentile(&values, 100.0), 5.0);
    }

    #[test]
    fn test_resource_alerts() {
        let monitor = SystemMonitor::new();
        let mut metrics = monitor.collect_metrics();
        
        metrics.resource_usage.cpu_usage_percent = 95.0;
        let alerts = monitor.check_resource_alerts(&metrics);
        assert!(alerts.iter().any(|alert| alert.contains("High CPU usage")));
    }
}