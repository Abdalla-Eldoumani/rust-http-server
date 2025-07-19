# Configuration System

The Rust HTTP Server uses a flexible configuration system that supports multiple sources with the following precedence order:

1. **Default values** (embedded in code)
2. **Configuration file** (`config.toml`)
3. **Environment variables** (prefixed with `APP_`)

## Configuration File

Create a `config.toml` file in the project root to customize settings:

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 2000
request_timeout_seconds = 60
shutdown_timeout_seconds = 30

[database]
url = "sqlite:./production.db"
max_connections = 20
min_connections = 2
connection_timeout_seconds = 30
migrate_on_start = true

[auth]
jwt_secret = "your-super-secret-jwt-key-here"
jwt_expiration_hours = 24
jwt_refresh_expiration_days = 7
password_min_length = 8
max_login_attempts = 5
lockout_duration_minutes = 15

[files]
upload_dir = "./uploads"
max_file_size_mb = 50
allowed_extensions = ["jpg", "jpeg", "png", "gif", "pdf", "txt", "doc", "docx", "zip"]
temp_dir = "./temp"

[cache]
max_size = 5000
default_ttl_seconds = 7200  # 2 hours
cleanup_interval_seconds = 600  # 10 minutes

[jobs]
max_workers = 8
queue_size = 5000
job_timeout_seconds = 1800  # 30 minutes
retry_attempts = 3
retry_delay_seconds = 120

[websocket]
max_connections = 2000
ping_interval_seconds = 30
pong_timeout_seconds = 10
message_buffer_size = 2048
```

## Environment Variables

Override any configuration value using environment variables with the `APP_` prefix:

```bash
# Server configuration
export APP_SERVER__HOST="0.0.0.0"
export APP_SERVER__PORT="8080"
export APP_SERVER__MAX_CONNECTIONS="2000"

# Database configuration
export APP_DATABASE__URL="sqlite:./production.db"
export APP_DATABASE__MAX_CONNECTIONS="20"

# Authentication configuration
export APP_AUTH__JWT_SECRET=""
export APP_AUTH__JWT_EXPIRATION_HOURS="48"

# File configuration
export APP_FILES__MAX_FILE_SIZE_MB="100"
export APP_FILES__UPLOAD_DIR="./production-uploads"

# Cache configuration
export APP_CACHE__MAX_SIZE="10000"

# Job configuration
export APP_JOBS__MAX_WORKERS="16"

# WebSocket configuration
export APP_WEBSOCKET__MAX_CONNECTIONS="5000"
```

## Default Values

If no configuration file or environment variables are provided, the system uses these defaults:

- **Server**: `127.0.0.1:3000`, max 1000 connections
- **Database**: SQLite at `./data.db`, max 10 connections
- **Auth**: Default JWT secret (⚠️ **change in production!**), 24h expiration
- **Files**: `./uploads` directory, 10MB max size
- **Cache**: 1000 items max, 1 hour TTL
- **Jobs**: 4 workers, 1000 queue size
- **WebSocket**: 1000 max connections

## Configuration Validation

The system validates all configuration values on startup:

- Server port must be > 0
- Database URL cannot be empty
- JWT secret cannot be empty
- Password minimum length must be ≥ 6
- All numeric limits must be > 0

Invalid configurations will prevent the server from starting with clear error messages.

## Directory Creation

The configuration system automatically creates required directories:

- Upload directory (`files.upload_dir`)
- Temporary directory (`files.temp_dir`)

## Usage in Code

```rust
use core_lib::config::AppConfig;

// Load configuration from all sources
let config = AppConfig::load()?;

// Access configuration values
println!("Server will bind to: {}", config.bind_address());
println!("Database URL: {}", config.database.url);
println!("Max file size: {}MB", config.files.max_file_size_mb);

// Create required directories
config.create_directories()?;

// Validate configuration
config.validate()?;
```