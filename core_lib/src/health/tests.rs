#[cfg(test)]
mod tests {
    use crate::health::checks::{
        HealthChecker, HealthStatus, ComponentHealth, SystemHealth,
        DatabaseHealthCheck, FilesystemHealthCheck, DependencyHealthCheck, HealthCheck,
    };
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_component_health_creation() {
        let health = ComponentHealth::healthy("All good".to_string(), 100);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.message, "All good");
        assert_eq!(health.response_time_ms, 100);
        assert!(health.details.is_none());

        let health = ComponentHealth::degraded("Slow response".to_string(), 1500);
        assert_eq!(health.status, HealthStatus::Degraded);
        assert_eq!(health.message, "Slow response");
        assert_eq!(health.response_time_ms, 1500);

        let health = ComponentHealth::unhealthy("Connection failed".to_string(), 5000);
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert_eq!(health.message, "Connection failed");
        assert_eq!(health.response_time_ms, 5000);
    }

    #[test]
    fn test_component_health_with_details() {
        let details = serde_json::json!({
            "error_code": 500,
            "retry_count": 3
        });

        let health = ComponentHealth::unhealthy("Service failed".to_string(), 2000)
            .with_details(details.clone());

        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert_eq!(health.details, Some(details));
    }

    #[test]
    fn test_system_health_creation() {
        let system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        assert_eq!(system_health.overall_status, HealthStatus::Healthy);
        assert_eq!(system_health.version, "1.0.0");
        assert_eq!(system_health.uptime_seconds, 3600);
        assert!(system_health.components.is_empty());
        assert!(system_health.is_healthy());
    }

    #[test]
    fn test_system_health_add_healthy_component() {
        let mut system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        let component_health = ComponentHealth::healthy("Database OK".to_string(), 50);
        
        system_health.add_component("database".to_string(), component_health);
        
        assert_eq!(system_health.overall_status, HealthStatus::Healthy);
        assert_eq!(system_health.components.len(), 1);
        assert!(system_health.is_healthy());
    }

    #[test]
    fn test_system_health_add_degraded_component() {
        let mut system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        let component_health = ComponentHealth::degraded("Database slow".to_string(), 1500);
        
        system_health.add_component("database".to_string(), component_health);
        
        assert_eq!(system_health.overall_status, HealthStatus::Degraded);
        assert!(!system_health.is_healthy());
    }

    #[test]
    fn test_system_health_add_unhealthy_component() {
        let mut system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        let component_health = ComponentHealth::unhealthy("Database failed".to_string(), 5000);
        
        system_health.add_component("database".to_string(), component_health);
        
        assert_eq!(system_health.overall_status, HealthStatus::Unhealthy);
        assert!(!system_health.is_healthy());
    }

    #[test]
    fn test_system_health_mixed_components() {
        let mut system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        
        let healthy_component = ComponentHealth::healthy("Cache OK".to_string(), 25);
        system_health.add_component("cache".to_string(), healthy_component);
        assert_eq!(system_health.overall_status, HealthStatus::Healthy);
        
        let degraded_component = ComponentHealth::degraded("Database slow".to_string(), 1200);
        system_health.add_component("database".to_string(), degraded_component);
        assert_eq!(system_health.overall_status, HealthStatus::Degraded);
        
        let unhealthy_component = ComponentHealth::unhealthy("Service down".to_string(), 5000);
        system_health.add_component("external_service".to_string(), unhealthy_component);
        assert_eq!(system_health.overall_status, HealthStatus::Unhealthy);
    }

    async fn setup_test_db() -> SqlitePool {
        SqlitePool::connect(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_database_health_check_success() {
        let pool = setup_test_db().await;
        let health_check = DatabaseHealthCheck::new(pool);
        
        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.message.contains("Database connection successful"));
        assert_eq!(health_check.name(), "database");
    }

    #[tokio::test]
    async fn test_database_health_check_slow_response() {
        let pool = setup_test_db().await;
        let health_check = DatabaseHealthCheck::new(pool);
        
        let result = health_check.check().await;
        
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.response_time_ms < 1000);
    }

    #[tokio::test]
    async fn test_filesystem_health_check_success() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();
        
        let health_check = FilesystemHealthCheck::new(vec![temp_path.clone()]);
        let result = health_check.check().await;
        
        assert_eq!(result.status, HealthStatus::Healthy);
        assert!(result.message.contains("All filesystem paths accessible"));
        assert_eq!(health_check.name(), "filesystem");
        
        if let Some(details) = result.details {
            let path_details = details.get(&temp_path).unwrap();
            assert_eq!(path_details.get("exists").unwrap(), &serde_json::Value::Bool(true));
            assert_eq!(path_details.get("readable").unwrap(), &serde_json::Value::Bool(true));
            assert_eq!(path_details.get("writable").unwrap(), &serde_json::Value::Bool(true));
        }
    }

    #[tokio::test]
    async fn test_filesystem_health_check_nonexistent_path() {
        let nonexistent_path = "/this/path/does/not/exist".to_string();
        
        let health_check = FilesystemHealthCheck::new(vec![nonexistent_path.clone()]);
        let result = health_check.check().await;
        
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert!(result.message.contains("Filesystem access failed"));
        
        if let Some(details) = result.details {
            let issues = details.get("issues").unwrap().as_array().unwrap();
            assert!(issues.iter().any(|issue| 
                issue.as_str().unwrap().contains("Path does not exist")
            ));
        }
    }

    #[tokio::test]
    async fn test_filesystem_health_check_mixed_paths() {
        let temp_dir = TempDir::new().unwrap();
        let valid_path = temp_dir.path().to_string_lossy().to_string();
        let invalid_path = "/this/path/does/not/exist".to_string();
        
        let health_check = FilesystemHealthCheck::new(vec![valid_path, invalid_path]);
        let result = health_check.check().await;
        
        assert_eq!(result.status, HealthStatus::Degraded);
        assert!(result.message.contains("Some filesystem issues"));
    }

    #[tokio::test]
    async fn test_dependency_health_check_success() {
        let health_check = DependencyHealthCheck::new(
            "test_service".to_string(),
            || Ok("Service is running".to_string())
        );
        
        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Healthy);
        assert_eq!(result.message, "Service is running");
        assert_eq!(health_check.name(), "test_service");
    }

    #[tokio::test]
    async fn test_dependency_health_check_failure() {
        let health_check = DependencyHealthCheck::new(
            "failing_service".to_string(),
            || Err(crate::error::AppError::InternalServerError)
        );
        
        let result = health_check.check().await;
        assert_eq!(result.status, HealthStatus::Unhealthy);
        assert!(result.message.contains("Dependency check failed"));
        
        if let Some(details) = result.details {
            assert_eq!(details.get("dependency").unwrap(), "failing_service");
        }
    }

    #[tokio::test]
    async fn test_health_checker_creation() {
        let checker = HealthChecker::new("1.0.0".to_string());
        let system_health = checker.check_all().await;
        
        assert_eq!(system_health.version, "1.0.0");
        assert_eq!(system_health.overall_status, HealthStatus::Healthy);
        assert!(system_health.components.is_empty());
    }

    #[tokio::test]
    async fn test_health_checker_with_checks() {
        let pool = setup_test_db().await;
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path().to_string_lossy().to_string();
        
        let checker = HealthChecker::new("1.0.0".to_string())
            .add_check(DatabaseHealthCheck::new(pool))
            .add_check(FilesystemHealthCheck::new(vec![temp_path]))
            .add_check(DependencyHealthCheck::new(
                "test_service".to_string(),
                || Ok("Service OK".to_string())
            ));
        
        let system_health = checker.check_all().await;
        
        assert_eq!(system_health.overall_status, HealthStatus::Healthy);
        assert_eq!(system_health.components.len(), 3);
        assert!(system_health.components.contains_key("database"));
        assert!(system_health.components.contains_key("filesystem"));
        assert!(system_health.components.contains_key("test_service"));
    }

    #[tokio::test]
    async fn test_health_checker_with_failing_check() {
        let checker = HealthChecker::new("1.0.0".to_string())
            .add_check(DependencyHealthCheck::new(
                "healthy_service".to_string(),
                || Ok("Service OK".to_string())
            ))
            .add_check(DependencyHealthCheck::new(
                "failing_service".to_string(),
                || Err(crate::error::AppError::InternalServerError)
            ));
        
        let system_health = checker.check_all().await;
        
        assert_eq!(system_health.overall_status, HealthStatus::Unhealthy);
        assert_eq!(system_health.components.len(), 2);
        
        let healthy_component = system_health.components.get("healthy_service").unwrap();
        assert_eq!(healthy_component.status, HealthStatus::Healthy);
        
        let failing_component = system_health.components.get("failing_service").unwrap();
        assert_eq!(failing_component.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_health_checker_check_specific_component() {
        let checker = HealthChecker::new("1.0.0".to_string())
            .add_check(DependencyHealthCheck::new(
                "test_service".to_string(),
                || Ok("Service OK".to_string())
            ));
        
        let result = checker.check_component("test_service").await;
        assert!(result.is_some());
        
        let component_health = result.unwrap();
        assert_eq!(component_health.status, HealthStatus::Healthy);
        assert_eq!(component_health.message, "Service OK");
        
        let nonexistent_result = checker.check_component("nonexistent_service").await;
        assert!(nonexistent_result.is_none());
    }

    #[test]
    fn test_health_status_equality() {
        assert_eq!(HealthStatus::Healthy, HealthStatus::Healthy);
        assert_eq!(HealthStatus::Degraded, HealthStatus::Degraded);
        assert_eq!(HealthStatus::Unhealthy, HealthStatus::Unhealthy);
        
        assert_ne!(HealthStatus::Healthy, HealthStatus::Degraded);
        assert_ne!(HealthStatus::Degraded, HealthStatus::Unhealthy);
        assert_ne!(HealthStatus::Healthy, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_component_health_serialization() {
        let health = ComponentHealth::healthy("Test message".to_string(), 100)
            .with_details(serde_json::json!({"key": "value"}));
        
        let serialized = serde_json::to_string(&health).unwrap();
        let deserialized: ComponentHealth = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.status, health.status);
        assert_eq!(deserialized.message, health.message);
        assert_eq!(deserialized.response_time_ms, health.response_time_ms);
        assert_eq!(deserialized.details, health.details);
    }

    #[test]
    fn test_system_health_serialization() {
        let mut system_health = SystemHealth::new("1.0.0".to_string(), 3600);
        system_health.add_component(
            "test_component".to_string(),
            ComponentHealth::healthy("All good".to_string(), 50)
        );
        
        let serialized = serde_json::to_string(&system_health).unwrap();
        let deserialized: SystemHealth = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(deserialized.overall_status, system_health.overall_status);
        assert_eq!(deserialized.version, system_health.version);
        assert_eq!(deserialized.uptime_seconds, system_health.uptime_seconds);
        assert_eq!(deserialized.components.len(), system_health.components.len());
    }
}