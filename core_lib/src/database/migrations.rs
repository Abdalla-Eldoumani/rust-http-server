use sqlx::{SqlitePool, Row};
use tracing::{info, error};
use crate::error::{AppError, Result};
use chrono::{DateTime, Utc};

pub struct MigrationManager {
    pool: SqlitePool,
}

impl MigrationManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(&self) -> Result<()> {
        info!("Starting database migrations");

        self.create_migrations_table().await?;

        let current_version = self.get_current_version().await?;
        info!("Current migration version: {}", current_version);

        let migrations = self.get_migrations();
        let mut applied_count = 0;

        for migration in migrations {
            if migration.version > current_version {
                info!("Applying migration {}: {}", migration.version, migration.name);
                self.apply_migration(&migration).await?;
                applied_count += 1;
            }
        }

        if applied_count > 0 {
            info!("Applied {} migrations successfully", applied_count);
        } else {
            info!("No new migrations to apply");
        }

        Ok(())
    }

    async fn create_migrations_table(&self) -> Result<()> {
        sqlx::query(r#"
            CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                checksum TEXT NOT NULL
            )
        "#)
        .execute(&self.pool)
        .await
        .map_err(AppError::from)?;

        Ok(())
    }

    async fn get_current_version(&self) -> Result<i64> {
        let result = sqlx::query("SELECT MAX(version) as version FROM _migrations")
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;

        match result {
            Some(row) => Ok(row.try_get("version").unwrap_or(0)),
            None => Ok(0),
        }
    }

    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(AppError::from)?;

        for statement in &migration.sql_statements {
            sqlx::query(statement)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    error!("Failed to execute migration statement: {}", e);
                    AppError::from(e)
                })?;
        }

        sqlx::query(r#"
            INSERT INTO _migrations (version, name, checksum)
            VALUES (?, ?, ?)
        "#)
        .bind(migration.version)
        .bind(&migration.name)
        .bind(&migration.checksum)
        .execute(&mut *tx)
        .await
        .map_err(AppError::from)?;

        tx.commit().await.map_err(AppError::from)?;
        Ok(())
    }

    fn get_migrations(&self) -> Vec<Migration> {
        vec![
            Migration {
                version: 1,
                name: "create_items_table".to_string(),
                checksum: "items_v1".to_string(),
                sql_statements: vec![
                    r#"
                    CREATE TABLE items (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL,
                        description TEXT,
                        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                        updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                        tags TEXT DEFAULT '[]',
                        metadata TEXT DEFAULT '{}',
                        created_by INTEGER,
                        FOREIGN KEY (created_by) REFERENCES users(id)
                    )
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_items_name ON items(name)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_items_created_at ON items(created_at)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_items_updated_at ON items(updated_at)
                    "#.to_string(),
                ],
            },
            Migration {
                version: 2,
                name: "create_users_table".to_string(),
                checksum: "users_v1".to_string(),
                sql_statements: vec![
                    r#"
                    CREATE TABLE users (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        username TEXT NOT NULL UNIQUE,
                        email TEXT NOT NULL UNIQUE,
                        password_hash TEXT NOT NULL,
                        role TEXT NOT NULL DEFAULT 'User',
                        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                        last_login DATETIME,
                        is_active BOOLEAN NOT NULL DEFAULT 1
                    )
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_users_username ON users(username)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_users_email ON users(email)
                    "#.to_string(),
                ],
            },
            Migration {
                version: 3,
                name: "create_files_table".to_string(),
                checksum: "files_v1".to_string(),
                sql_statements: vec![
                    r#"
                    CREATE TABLE files (
                        id TEXT PRIMARY KEY,
                        filename TEXT NOT NULL,
                        original_filename TEXT NOT NULL,
                        content_type TEXT NOT NULL,
                        size INTEGER NOT NULL,
                        path TEXT NOT NULL,
                        uploaded_by INTEGER NOT NULL,
                        created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                        item_id INTEGER,
                        FOREIGN KEY (uploaded_by) REFERENCES users(id),
                        FOREIGN KEY (item_id) REFERENCES items(id) ON DELETE SET NULL
                    )
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_files_item_id ON files(item_id)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_files_uploaded_by ON files(uploaded_by)
                    "#.to_string(),
                ],
            },
            Migration {
                version: 4,
                name: "create_jobs_table".to_string(),
                checksum: "jobs_v2".to_string(),
                sql_statements: vec![
                    r#"
                    CREATE TABLE jobs (
                        id TEXT PRIMARY KEY,
                        job_type TEXT NOT NULL,
                        status TEXT NOT NULL,
                        payload TEXT NOT NULL,
                        result TEXT,
                        error_message TEXT,
                        created_at TEXT NOT NULL,
                        started_at TEXT,
                        completed_at TEXT,
                        retry_count INTEGER NOT NULL DEFAULT 0,
                        max_retries INTEGER NOT NULL DEFAULT 3,
                        priority TEXT NOT NULL DEFAULT 'normal'
                    )
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_jobs_status ON jobs(status)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_jobs_type ON jobs(job_type)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_jobs_created_at ON jobs(created_at)
                    "#.to_string(),
                    r#"
                    CREATE INDEX idx_jobs_priority ON jobs(priority)
                    "#.to_string(),
                ],
            },
            Migration {
                version: 5,
                name: "create_fts_search".to_string(),
                checksum: "fts_v1".to_string(),
                sql_statements: vec![
                    r#"
                    CREATE VIRTUAL TABLE items_fts USING fts5(
                        name, 
                        description, 
                        content='items', 
                        content_rowid='id'
                    )
                    "#.to_string(),
                    r#"
                    CREATE TRIGGER items_fts_insert AFTER INSERT ON items BEGIN
                        INSERT INTO items_fts(rowid, name, description) 
                        VALUES (new.id, new.name, new.description);
                    END
                    "#.to_string(),
                    r#"
                    CREATE TRIGGER items_fts_delete AFTER DELETE ON items BEGIN
                        INSERT INTO items_fts(items_fts, rowid, name, description) 
                        VALUES('delete', old.id, old.name, old.description);
                    END
                    "#.to_string(),
                    r#"
                    CREATE TRIGGER items_fts_update AFTER UPDATE ON items BEGIN
                        INSERT INTO items_fts(items_fts, rowid, name, description) 
                        VALUES('delete', old.id, old.name, old.description);
                        INSERT INTO items_fts(rowid, name, description) 
                        VALUES (new.id, new.name, new.description);
                    END
                    "#.to_string(),
                ],
            },
            Migration {
                version: 6,
                name: "update_jobs_table".to_string(),
                checksum: "jobs_update_v2".to_string(),
                sql_statements: vec![
                    r#"
                    -- Create new jobs table with all required columns
                    CREATE TABLE IF NOT EXISTS jobs_new (
                        id TEXT PRIMARY KEY,
                        job_type TEXT NOT NULL,
                        status TEXT NOT NULL,
                        payload TEXT NOT NULL,
                        result TEXT,
                        error_message TEXT,
                        created_at TEXT NOT NULL,
                        started_at TEXT,
                        completed_at TEXT,
                        retry_count INTEGER NOT NULL DEFAULT 0,
                        max_retries INTEGER NOT NULL DEFAULT 3,
                        priority TEXT NOT NULL DEFAULT 'normal'
                    )
                    "#.to_string(),
                    r#"
                    -- Copy data from old table if it exists
                    INSERT OR IGNORE INTO jobs_new (id, job_type, status, payload, error_message, created_at, started_at, completed_at)
                    SELECT id, job_type, status, payload, 
                           COALESCE(error_message, '') as error_message,
                           created_at, started_at, completed_at
                    FROM jobs WHERE EXISTS (SELECT name FROM sqlite_master WHERE type='table' AND name='jobs')
                    "#.to_string(),
                    r#"
                    -- Drop old table if it exists
                    DROP TABLE IF EXISTS jobs
                    "#.to_string(),
                    r#"
                    -- Rename new table to jobs
                    ALTER TABLE jobs_new RENAME TO jobs
                    "#.to_string(),
                    r#"
                    CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)
                    "#.to_string(),
                    r#"
                    CREATE INDEX IF NOT EXISTS idx_jobs_type ON jobs(job_type)
                    "#.to_string(),
                    r#"
                    CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at)
                    "#.to_string(),
                    r#"
                    CREATE INDEX IF NOT EXISTS idx_jobs_priority ON jobs(priority)
                    "#.to_string(),
                ],
            },
        ]
    }

    pub async fn get_migration_history(&self) -> Result<Vec<MigrationRecord>> {
        let rows = sqlx::query(r#"
            SELECT version, name, applied_at, checksum
            FROM _migrations
            ORDER BY version
        "#)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::from)?;

        let mut records = Vec::new();
        for row in rows {
            records.push(MigrationRecord {
                version: row.try_get("version").unwrap_or(0),
                name: row.try_get("name").unwrap_or_default(),
                applied_at: row.try_get("applied_at").unwrap_or_else(|_| Utc::now()),
                checksum: row.try_get("checksum").unwrap_or_default(),
            });
        }

        Ok(records)
    }
}

#[derive(Debug, Clone)]
struct Migration {
    version: i64,
    name: String,
    checksum: String,
    sql_statements: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MigrationRecord {
    pub version: i64,
    pub name: String,
    pub applied_at: DateTime<Utc>,
    pub checksum: String,
}

pub async fn run_migrations(pool: SqlitePool) -> Result<()> {
    let migration_manager = MigrationManager::new(pool);
    migration_manager.run_migrations().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use crate::database::connection::get_database_pool;

    #[tokio::test]
    async fn test_migrations() {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = get_database_pool(&database_url).await.unwrap();
        let migration_manager = MigrationManager::new(pool.clone());
        
        migration_manager.run_migrations().await.unwrap();
        
        let row = sqlx::query("SELECT COUNT(*) as count FROM sqlite_master WHERE type='table' AND name IN ('items', 'users', 'files', 'jobs')")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        let table_count: i64 = row.try_get("count").unwrap();
        assert_eq!(table_count, 4);
        
        let history = migration_manager.get_migration_history().await.unwrap();
        assert_eq!(history.len(), 6);
    }
}