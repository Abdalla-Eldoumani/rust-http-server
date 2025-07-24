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
    pub cors: CorsConfig,
    pub rate_limit: RateLimitConfig,
    pub logging: LoggingConfig,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub exposed_headers: Vec<String>,
    pub allow_credentials: bool,
    pub max_age_seconds: u64,
    pub enable_permissive_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub enable: bool,
    pub requests_per_minute: usize,
    pub burst_size: usize,
    pub enable_user_based_limits: bool,
    pub user_requests_per_minute: usize,
    pub admin_requests_per_minute: usize,
    pub cleanup_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub include_request_id: bool,
    pub include_user_info: bool,
    pub include_timing: bool,
    pub log_request_body: bool,
    pub log_response_body: bool,
    pub max_body_size: usize,
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
            cors: CorsConfig::default(),
            rate_limit: RateLimitConfig::default(),
            logging: LoggingConfig::default(),
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

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec![
                "http://localhost:3000".to_string(),
                "http://localhost:3001".to_string(),
                "http://localhost:5173".to_string(),
                "http://localhost:5174".to_string(),
                "http://localhost:8080".to_string(),
                "http://127.0.0.1:3000".to_string(),
                "http://127.0.0.1:8080".to_string(),
                "https://localhost:3000".to_string(),
                "https://localhost:8080".to_string(),
            ],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
                "DELETE".to_string(),
                "PATCH".to_string(),
                "HEAD".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "content-type".to_string(),
                "authorization".to_string(),
                "accept".to_string(),
                "x-requested-with".to_string(),
                "user-agent".to_string(),
                "origin".to_string(),
                "referer".to_string(),
                "cache-control".to_string(),
            ],
            exposed_headers: vec![
                "x-request-id".to_string(),
                "x-response-time".to_string(),
                "x-ratelimit-limit".to_string(),
                "x-ratelimit-remaining".to_string(),
            ],
            allow_credentials: true,
            max_age_seconds: 3600,
            enable_permissive_mode: false,
        }
    }
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enable: true,
            requests_per_minute: 60,
            burst_size: 10,
            enable_user_based_limits: true,
            user_requests_per_minute: 100,
            admin_requests_per_minute: 200,
            cleanup_interval_seconds: 300,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            include_request_id: true,
            include_user_info: true,
            include_timing: true,
            log_request_body: false,
            log_response_body: false,
            max_body_size: 1024,
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

        if self.rate_limit.enable && self.rate_limit.requests_per_minute == 0 {
            return Err(ConfigError::Message(
                "Rate limit requests per minute must be greater than 0".to_string(),
            ));
        }

        if self.rate_limit.enable && self.rate_limit.burst_size == 0 {
            return Err(ConfigError::Message(
                "Rate limit burst size must be greater than 0".to_string(),
            ));
        }

        if !["debug", "info", "warn", "error"].contains(&self.logging.level.as_str()) {
            return Err(ConfigError::Message(
                "Logging level must be one of: debug, info, warn, error".to_string(),
            ));
        }

        if !["json", "pretty"].contains(&self.logging.format.as_str()) {
            return Err(ConfigError::Message(
                "Logging format must be either 'json' or 'pretty'".to_string(),
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
        assert_eq!(config.cors.allow_credentials, true);
        assert_eq!(config.rate_limit.enable, true);
        assert_eq!(config.logging.level, "info");
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
        assert_eq!(config.cors.max_age_seconds, 3600);
        assert_eq!(config.rate_limit.requests_per_minute, 60);
        assert_eq!(config.logging.format, "pretty");
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