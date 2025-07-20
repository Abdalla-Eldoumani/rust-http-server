use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{SqlitePool, Row};
use uuid::Uuid;

use crate::error::{AppError, Result};
use super::models::{File, FileListQuery};

#[async_trait]
pub trait FileRepositoryTrait: Send + Sync {
    async fn create(&self, file: &File) -> Result<File>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<File>>;
    async fn update(&self, file: &File) -> Result<File>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn list(&self, query: &FileListQuery) -> Result<Vec<File>>;
    async fn get_by_item_id(&self, item_id: u64) -> Result<Vec<File>>;
    async fn count(&self, query: &FileListQuery) -> Result<u64>;
}

#[derive(Clone)]
pub struct FileRepository {
    pool: SqlitePool,
}

impl FileRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    pub async fn create_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS files (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                original_filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                size INTEGER NOT NULL,
                path TEXT NOT NULL,
                uploaded_by INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                item_id INTEGER,
                FOREIGN KEY (uploaded_by) REFERENCES users (id),
                FOREIGN KEY (item_id) REFERENCES items (id) ON DELETE SET NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_uploaded_by ON files (uploaded_by)")
            .execute(&self.pool)
            .await?;
            
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_item_id ON files (item_id)")
            .execute(&self.pool)
            .await?;
            
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_content_type ON files (content_type)")
            .execute(&self.pool)
            .await?;
            
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_created_at ON files (created_at)")
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
}

#[async_trait]
impl FileRepositoryTrait for FileRepository {
    async fn create(&self, file: &File) -> Result<File> {
        sqlx::query(
            r#"
            INSERT INTO files (id, filename, original_filename, content_type, size, path, uploaded_by, created_at, item_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(file.id.to_string())
        .bind(&file.filename)
        .bind(&file.original_filename)
        .bind(&file.content_type)
        .bind(file.size as i64)
        .bind(&file.path)
        .bind(file.uploaded_by as i64)
        .bind(file.created_at.to_rfc3339())
        .bind(file.item_id.map(|id| id as i64))
        .execute(&self.pool)
        .await?;
        
        Ok(file.clone())
    }
    
    async fn get_by_id(&self, id: Uuid) -> Result<Option<File>> {
        let row = sqlx::query(
            "SELECT id, filename, original_filename, content_type, size, path, uploaded_by, created_at, item_id FROM files WHERE id = ?1"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(row) => {
                let file = File {
                    id: Uuid::parse_str(&row.get::<String, _>("id"))
                        .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?,
                    filename: row.get("filename"),
                    original_filename: row.get("original_filename"),
                    content_type: row.get("content_type"),
                    size: row.get::<i64, _>("size") as u64,
                    path: row.get("path"),
                    uploaded_by: row.get::<i64, _>("uploaded_by") as u64,
                    created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                        .map_err(|e| AppError::BadRequest(format!("Invalid datetime: {}", e)))?
                        .with_timezone(&Utc),
                    item_id: row.get::<Option<i64>, _>("item_id").map(|id| id as u64),
                };
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }
    
    async fn update(&self, file: &File) -> Result<File> {
        let rows_affected = sqlx::query(
            r#"
            UPDATE files 
            SET filename = ?2, original_filename = ?3, content_type = ?4, size = ?5, 
                path = ?6, uploaded_by = ?7, created_at = ?8, item_id = ?9
            WHERE id = ?1
            "#,
        )
        .bind(file.id.to_string())
        .bind(&file.filename)
        .bind(&file.original_filename)
        .bind(&file.content_type)
        .bind(file.size as i64)
        .bind(&file.path)
        .bind(file.uploaded_by as i64)
        .bind(file.created_at.to_rfc3339())
        .bind(file.item_id.map(|id| id as i64))
        .execute(&self.pool)
        .await?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(AppError::NotFound("File not found".to_string()));
        }
        
        Ok(file.clone())
    }
    
    async fn delete(&self, id: Uuid) -> Result<()> {
        let rows_affected = sqlx::query("DELETE FROM files WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?
            .rows_affected();
        
        if rows_affected == 0 {
            return Err(AppError::NotFound("File not found".to_string()));
        }
        
        Ok(())
    }
    
    async fn list(&self, query: &FileListQuery) -> Result<Vec<File>> {
        let mut sql = "SELECT id, filename, original_filename, content_type, size, path, uploaded_by, created_at, item_id FROM files WHERE 1=1".to_string();
        let mut conditions = Vec::new();
        
        if query.item_id.is_some() {
            conditions.push("item_id = ?".to_string());
        }
        
        if query.content_type.is_some() {
            conditions.push("content_type = ?".to_string());
        }
        
        if query.uploaded_by.is_some() {
            conditions.push("uploaded_by = ?".to_string());
        }
        
        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }
        
        sql.push_str(" ORDER BY created_at DESC");
        
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
        
        let mut query_builder = sqlx::query(&sql);
        
        if let Some(item_id) = query.item_id {
            query_builder = query_builder.bind(item_id as i64);
        }
        
        if let Some(ref content_type) = query.content_type {
            query_builder = query_builder.bind(content_type);
        }
        
        if let Some(uploaded_by) = query.uploaded_by {
            query_builder = query_builder.bind(uploaded_by as i64);
        }
        
        let rows = query_builder.fetch_all(&self.pool).await?;
        
        let mut files = Vec::new();
        for row in rows {
            let file = File {
                id: Uuid::parse_str(&row.get::<String, _>("id"))
                    .map_err(|e| AppError::BadRequest(format!("Invalid UUID: {}", e)))?,
                filename: row.get("filename"),
                original_filename: row.get("original_filename"),
                content_type: row.get("content_type"),
                size: row.get::<i64, _>("size") as u64,
                path: row.get("path"),
                uploaded_by: row.get::<i64, _>("uploaded_by") as u64,
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .map_err(|e| AppError::BadRequest(format!("Invalid datetime: {}", e)))?
                    .with_timezone(&Utc),
                item_id: row.get::<Option<i64>, _>("item_id").map(|id| id as u64),
            };
            files.push(file);
        }
        
        Ok(files)
    }
    
    async fn get_by_item_id(&self, item_id: u64) -> Result<Vec<File>> {
        let query = FileListQuery {
            item_id: Some(item_id),
            ..Default::default()
        };
        self.list(&query).await
    }
    
    async fn count(&self, query: &FileListQuery) -> Result<u64> {
        let mut sql = "SELECT COUNT(*) as count FROM files WHERE 1=1".to_string();
        let mut conditions = Vec::new();
        
        if query.item_id.is_some() {
            conditions.push("item_id = ?".to_string());
        }
        
        if query.content_type.is_some() {
            conditions.push("content_type = ?".to_string());
        }
        
        if query.uploaded_by.is_some() {
            conditions.push("uploaded_by = ?".to_string());
        }
        
        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }
        
        let mut query_builder = sqlx::query(&sql);
        
        if let Some(item_id) = query.item_id {
            query_builder = query_builder.bind(item_id as i64);
        }
        
        if let Some(ref content_type) = query.content_type {
            query_builder = query_builder.bind(content_type);
        }
        
        if let Some(uploaded_by) = query.uploaded_by {
            query_builder = query_builder.bind(uploaded_by as i64);
        }
        
        let row = query_builder.fetch_one(&self.pool).await?;
        let count = row.get::<i64, _>("count") as u64;
        
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use tempfile::NamedTempFile;
    
    async fn create_test_pool() -> SqlitePool {
        let temp_file = NamedTempFile::new().unwrap();
        let database_url = format!("sqlite:{}", temp_file.path().display());
        
        let pool = SqlitePool::connect(&database_url).await.unwrap();
        
        sqlx::query(
            r#"
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                username TEXT NOT NULL,
                email TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'user',
                created_at TEXT NOT NULL,
                last_login TEXT,
                is_active BOOLEAN NOT NULL DEFAULT 1
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        
        sqlx::query(
            r#"
            CREATE TABLE items (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                tags TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                created_by INTEGER,
                FOREIGN KEY (created_by) REFERENCES users (id)
            )
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO users (id, username, email, password_hash, created_at) VALUES (1, 'test', 'test@example.com', 'hash', datetime('now'))")
            .execute(&pool)
            .await
            .unwrap();
        
        pool
    }
    
    #[tokio::test]
    async fn test_file_repository_crud() {
        let pool = create_test_pool().await;
        let repo = FileRepository::new(pool);
        repo.create_table().await.unwrap();
        
        let file = File {
            id: Uuid::new_v4(),
            filename: "test.txt".to_string(),
            original_filename: "original_test.txt".to_string(),
            content_type: "text/plain".to_string(),
            size: 1024,
            path: "/uploads/test.txt".to_string(),
            uploaded_by: 1,
            created_at: Utc::now(),
            item_id: None,
        };
        
        let created = repo.create(&file).await.unwrap();
        assert_eq!(created.id, file.id);
        
        let retrieved = repo.get_by_id(file.id).await.unwrap().unwrap();
        assert_eq!(retrieved.filename, file.filename);
        
        let mut updated_file = retrieved.clone();
        updated_file.filename = "updated.txt".to_string();
        let updated = repo.update(&updated_file).await.unwrap();
        assert_eq!(updated.filename, "updated.txt");
        
        let files = repo.list(&FileListQuery::default()).await.unwrap();
        assert_eq!(files.len(), 1);
        
        repo.delete(file.id).await.unwrap();
        let deleted = repo.get_by_id(file.id).await.unwrap();
        assert!(deleted.is_none());
    }
}