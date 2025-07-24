use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use std::time::Duration;
use tracing::{info, error};
use crate::error::{AppError, Result};

#[derive(Clone)]
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn health_check(&self) -> Result<()> {
        let row = sqlx::query("SELECT 1 as test")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                error!("Database health check failed: {}", e);
                AppError::from(e)
            })?;

        let test_value: i32 = row.try_get("test")
            .map_err(AppError::from)?;

        if test_value == 1 {
            Ok(())
        } else {
            Err(AppError::from(sqlx::Error::RowNotFound))
        }
    }

    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let row = sqlx::query(r#"
            SELECT 
                (SELECT COUNT(*) FROM sqlite_master WHERE type='table') as table_count,
                (SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()) as db_size
        "#)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(DatabaseStats {
            table_count: row.try_get("table_count").unwrap_or(0),
            database_size_bytes: row.try_get("db_size").unwrap_or(0),
            connection_pool_size: self.pool.size() as i64,
            active_connections: self.pool.num_idle() as i64,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DatabaseStats {
    pub table_count: i64,
    pub database_size_bytes: i64,
    pub connection_pool_size: i64,
    pub active_connections: i64,
}

pub async fn get_database_pool(database_url: &str) -> Result<SqlitePool> {
    info!("Connecting to database: {}", database_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))
        .test_before_acquire(true)
        .connect(database_url)
        .await
        .map_err(|e| {
            error!("Failed to create database pool: {}", e);
            AppError::from(e)
        })?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    sqlx::query("PRAGMA synchronous = NORMAL")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    sqlx::query("PRAGMA busy_timeout = 30000")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    sqlx::query("PRAGMA read_uncommitted = OFF")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    sqlx::query("PRAGMA automatic_index = ON")
        .execute(&pool)
        .await
        .map_err(AppError::from)?;

    info!("Database connection pool created successfully");
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_database_connection() {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = get_database_pool(&database_url).await.unwrap();
        let db_manager = DatabaseManager::new(pool);
        
        db_manager.health_check().await.unwrap();
        
        let stats = db_manager.get_stats().await.unwrap();
        assert!(stats.connection_pool_size > 0);
    }
}