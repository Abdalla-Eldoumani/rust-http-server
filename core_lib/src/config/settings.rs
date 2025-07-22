use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub files: FileConfig,
    pub cache: CacheConfig,
    pub jobs: JobConfig,
    pub websocket: WebSocketConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub request_timeout_seconds: u64,
    pub shutdown_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub migrate_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub jwt_refresh_expiration_days: u64,
    pub password_min_length: usize,
    pub max_login_attempts: u32,
    pub lockout_duration_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileConfig {
    pub upload_dir: PathBuf,
    pub max_file_size_mb: u64,
    pub allowed_extensions: Vec<String>,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_size: usize,
    pub default_ttl_seconds: u64,
    pub cleanup_interval_seconds: u64,
    pub enable_stats: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub max_workers: usize,
    pub queue_size: usize,
    pub job_timeout_seconds: u64,
    pub retry_attempts: u32,
    pub retry_delay_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub max_connections: usize,
    pub ping_interval_seconds: u64,
    pub pong_timeout_seconds: u64,
    pub message_buffer_size: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            database: DatabaseConfig::default(),
            auth: AuthConfig::default(),
            files: FileConfig::default(),
            cache: CacheConfig::default(),
            jobs: JobConfig::default(),
            websocket: WebSocketConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            max_connections: 1000,
            request_timeout_seconds: 30,
            shutdown_timeout_seconds: 10,
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite:./data.db".to_string(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout_seconds: 30,
            migrate_on_start: true,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "1a9e1a1d8f3e9613a555adea1881bbd1".to_string(),
            jwt_expiration_hours: 24,
            jwt_refresh_expiration_days: 7,
            password_min_length: 8,
            max_login_attempts: 5,
            lockout_duration_minutes: 15,
        }
    }
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            upload_dir: PathBuf::from("./uploads"),
            max_file_size_mb: 10,
            allowed_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "png".to_string(),
                "gif".to_string(),
                "pdf".to_string(),
                "txt".to_string(),
                "doc".to_string(),
                "docx".to_string(),
            ],
            temp_dir: PathBuf::from("./temp"),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 1000,
            default_ttl_seconds: 3600,
            cleanup_interval_seconds: 300,
            enable_stats: true,
        }
    }
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            queue_size: 1000,
            job_timeout_seconds: 300,
            retry_attempts: 3,
            retry_delay_seconds: 60,
        }
    }
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            ping_interval_seconds: 30,
            pong_timeout_seconds: 10,
            message_buffer_size: 1024,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let mut builder = Config::builder()
            .add_source(Config::try_from(&AppConfig::default())?);

        if std::path::Path::new("config.toml").exists() {
            builder = builder.add_source(File::with_name("config"));
        }

        builder = builder.add_source(
            Environment::with_prefix("APP")
                .separator("_")
                .try_parsing(true),
        );

        let config = builder.build()?;
        let app_config: AppConfig = config.try_deserialize()?;

        app_config.validate()?;

        Ok(app_config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::Message("Server port cannot be 0".to_string()));
        }

        if self.server.max_connections == 0 {
            return Err(ConfigError::Message(
                "Max connections must be greater than 0".to_string(),
            ));
        }

        if self.database.url.is_empty() {
            return Err(ConfigError::Message(
                "Database URL cannot be empty".to_string(),
            ));
        }

        if self.database.max_connections == 0 {
            return Err(ConfigError::Message(
                "Database max connections must be greater than 0".to_string(),
            ));
        }

        if self.auth.jwt_secret.is_empty() {
            return Err(ConfigError::Message(
                "JWT secret cannot be empty".to_string(),
            ));
        }

        if self.auth.jwt_secret == "1a9e1a1d8f3e9613a555adea1881bbd1" {
            tracing::warn!("Using default JWT secret - change this in production!");
        }

        if self.auth.password_min_length < 6 {
            return Err(ConfigError::Message(
                "Password minimum length must be at least 6".to_string(),
            ));
        }

        if self.files.max_file_size_mb == 0 {
            return Err(ConfigError::Message(
                "Max file size must be greater than 0".to_string(),
            ));
        }

        if self.cache.max_size == 0 {
            return Err(ConfigError::Message(
                "Cache max size must be greater than 0".to_string(),
            ));
        }

        if self.jobs.max_workers == 0 {
            return Err(ConfigError::Message(
                "Job max workers must be greater than 0".to_string(),
            ));
        }

        if self.websocket.max_connections == 0 {
            return Err(ConfigError::Message(
                "WebSocket max connections must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    pub fn create_directories(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.files.upload_dir)?;
        std::fs::create_dir_all(&self.files.temp_dir)?;
        Ok(())
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.url, "sqlite:./data.db");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = AppConfig::default();
        
        config.server.port = 0;
        assert!(config.validate().is_err());
        
        config = AppConfig::default();
        config.auth.jwt_secret = String::new();
        assert!(config.validate().is_err());
        
        config = AppConfig::default();
        config.auth.password_min_length = 3;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_bind_address() {
        let config = AppConfig::default();
        assert_eq!(config.bind_address(), "127.0.0.1:3000");
        
        let mut config = AppConfig::default();
        config.server.host = "0.0.0.0".to_string();
        config.server.port = 8080;
        assert_eq!(config.bind_address(), "0.0.0.0:8080");
    }

    #[test]
    fn test_config_loading() {
        use std::env;
        
        env::remove_var("APP_SERVER_PORT");
        env::remove_var("APP_DATABASE_MAX_CONNECTIONS");
        env::remove_var("APP_AUTH_JWT_EXPIRATION_HOURS");
        
        let config = AppConfig::load().expect("Should load default configuration");
        
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.database.url, "sqlite:./data.db");
        assert_eq!(config.auth.jwt_expiration_hours, 24);
        assert_eq!(config.files.max_file_size_mb, 10);
        assert_eq!(config.cache.max_size, 1000);
        assert_eq!(config.jobs.max_workers, 4);
        assert_eq!(config.websocket.max_connections, 1000);
    }

    #[test]
    fn test_environment_variable_support() {
        let config = AppConfig::load().expect("Should load configuration");
        
        assert!(config.validate().is_ok());
        
        assert!(!config.server.host.is_empty());
        assert!(config.server.port > 0);
        assert!(!config.database.url.is_empty());
        assert!(config.database.max_connections > 0);
    }

    #[test]
    fn test_directory_creation() {
        let config = AppConfig::default();
        
        assert!(config.create_directories().is_ok());
        
        assert!(config.files.upload_dir.exists());
        assert!(config.files.temp_dir.exists());
    }
}