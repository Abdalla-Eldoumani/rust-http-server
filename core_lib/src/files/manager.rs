use std::path::{Path, PathBuf};
use std::fs;
use chrono::{Utc, Datelike};
use uuid::Uuid;
use tokio::fs as async_fs;
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, Result};
use super::models::{File, FileUpload, FileMetadata, FileListQuery};
use super::repository::{FileRepository, FileRepositoryTrait};
use super::validation::{FileValidator, FileValidationConfig};

#[derive(Clone)]
pub struct FileManagerConfig {
    pub storage_path: PathBuf,
    pub validation: FileValidationConfig,
    pub create_subdirectories: bool,
}

impl Default for FileManagerConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("uploads"),
            validation: FileValidationConfig::default(),
            create_subdirectories: true,
        }
    }
}

#[derive(Clone)]
pub struct FileManager {
    config: FileManagerConfig,
    repository: FileRepository,
    validator: FileValidator,
}

impl FileManager {
    pub fn new(config: FileManagerConfig, repository: FileRepository) -> Self {
        let validator = FileValidator::new(config.validation.clone());
        
        Self {
            config,
            repository,
            validator,
        }
    }
    
    pub fn with_default_config(repository: FileRepository) -> Self {
        Self::new(FileManagerConfig::default(), repository)
    }
    
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.storage_path.exists() {
            async_fs::create_dir_all(&self.config.storage_path).await?;
        }
        
        self.repository.create_table().await?;
        
        Ok(())
    }
    
    pub async fn store_file(&self, upload: FileUpload) -> Result<FileMetadata> {
        self.validator.validate_upload(
            &upload.original_filename,
            &upload.content_type,
            &upload.data,
        ).map_err(|e| AppError::BadRequest(e.to_string()))?;
        
        let file_id = Uuid::new_v4();
        let file_extension = Path::new(&upload.original_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let filename = if file_extension.is_empty() {
            file_id.to_string()
        } else {
            format!("{}.{}", file_id, file_extension)
        };
        
        let storage_path = if self.config.create_subdirectories {
            let now = Utc::now();
            let subdir = format!("{}/{:02}", now.year(), now.month());
            let full_subdir = self.config.storage_path.join(&subdir);
            
            if !full_subdir.exists() {
                async_fs::create_dir_all(&full_subdir).await?;
            }
            
            full_subdir.join(&filename)
        } else {
            self.config.storage_path.join(&filename)
        };
        
        let mut file = async_fs::File::create(&storage_path).await?;
        file.write_all(&upload.data).await?;
        file.sync_all().await?;
        
        let file_record = File {
            id: file_id,
            filename: filename.clone(),
            original_filename: upload.original_filename,
            content_type: upload.content_type,
            size: upload.data.len() as u64,
            path: storage_path.to_string_lossy().to_string(),
            uploaded_by: upload.uploaded_by,
            created_at: Utc::now(),
            item_id: upload.item_id,
        };
        
        let stored_file = self.repository.create(&file_record).await?;
        
        Ok(stored_file.into())
    }
    
    pub async fn get_file_metadata(&self, file_id: Uuid) -> Result<Option<FileMetadata>> {
        match self.repository.get_by_id(file_id).await? {
            Some(file) => Ok(Some(file.into())),
            None => Ok(None),
        }
    }
    
    pub async fn get_file_data(&self, file_id: Uuid) -> Result<Option<(FileMetadata, Vec<u8>)>> {
        match self.repository.get_by_id(file_id).await? {
            Some(file) => {
                let normalized_path = Path::new(&file.path);
                let data = async_fs::read(normalized_path).await.map_err(|e| {
                    tracing::error!("Failed to read file {}: {}", file.path, e);
                    AppError::InternalServerError
                })?;
                
                Ok(Some((file.into(), data)))
            }
            None => Ok(None),
        }
    }
    
    pub async fn delete_file(&self, file_id: Uuid) -> Result<()> {
        let file = match self.repository.get_by_id(file_id).await? {
            Some(file) => file,
            None => return Err(AppError::NotFound("File not found".to_string())),
        };
        
        if Path::new(&file.path).exists() {
            async_fs::remove_file(&file.path).await.map_err(|e| {
                tracing::error!("Failed to delete file {}: {}", file.path, e);
                AppError::InternalServerError
            })?;
        }
        
        self.repository.delete(file_id).await?;
        
        Ok(())
    }
    
    pub async fn list_files(&self, query: FileListQuery) -> Result<Vec<FileMetadata>> {
        let files = self.repository.list(&query).await?;
        Ok(files.into_iter().map(|f| f.into()).collect())
    }
    
    pub async fn count_files(&self, query: FileListQuery) -> Result<u64> {
        self.repository.count(&query).await
    }
    
    pub async fn get_files_by_item(&self, item_id: u64) -> Result<Vec<FileMetadata>> {
        let files = self.repository.get_by_item_id(item_id).await?;
        Ok(files.into_iter().map(|f| f.into()).collect())
    }
    
    pub async fn associate_with_item(&self, file_id: Uuid, item_id: Option<u64>) -> Result<FileMetadata> {
        let mut file = match self.repository.get_by_id(file_id).await? {
            Some(file) => file,
            None => return Err(AppError::NotFound("File not found".to_string())),
        };
        
        file.item_id = item_id;
        let updated_file = self.repository.update(&file).await?;
        
        Ok(updated_file.into())
    }
    
    pub async fn cleanup_orphaned_files(&self) -> Result<u64> {
        let mut cleaned_count = 0;
        
        let all_files = self.repository.list(&FileListQuery {
            limit: None,
            offset: None,
            ..Default::default()
        }).await?;
        
        for file in all_files {
            if !Path::new(&file.path).exists() {
                tracing::warn!("Removing orphaned database record for missing file: {}", file.path);
                if let Err(e) = self.repository.delete(file.id).await {
                    tracing::error!("Failed to remove orphaned record {}: {}", file.id, e);
                } else {
                    cleaned_count += 1;
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    pub fn get_storage_stats(&self) -> Result<StorageStats> {
        let mut total_size = 0;
        let mut file_count = 0;
        
        fn visit_dir(dir: &Path, total_size: &mut u64, file_count: &mut u64) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        visit_dir(&path, total_size, file_count)?;
                    } else {
                        *total_size += entry.metadata()?.len();
                        *file_count += 1;
                    }
                }
            }
            Ok(())
        }
        
        if self.config.storage_path.exists() {
            visit_dir(&self.config.storage_path, &mut total_size, &mut file_count)?;
        }
        
        Ok(StorageStats {
            total_size,
            file_count,
            storage_path: self.config.storage_path.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_size: u64,
    pub file_count: u64,
    pub storage_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use tempfile::{NamedTempFile, TempDir};
    
    async fn create_test_setup() -> (FileManager, TempDir) {
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
        
        let temp_dir = TempDir::new().unwrap();
        let repository = FileRepository::new(pool);
        
        let config = FileManagerConfig {
            storage_path: temp_dir.path().to_path_buf(),
            validation: FileValidationConfig::default(),
            create_subdirectories: false,
        };
        
        let manager = FileManager::new(config, repository);
        manager.initialize().await.unwrap();
        
        (manager, temp_dir)
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_file() {
        let (manager, _temp_dir) = create_test_setup().await;
        
        let upload = FileUpload {
            original_filename: "test.txt".to_string(),
            content_type: "text/plain".to_string(),
            data: b"Hello, World!".to_vec(),
            uploaded_by: 1,
            item_id: None,
        };
        
        let metadata = manager.store_file(upload).await.unwrap();
        assert_eq!(metadata.original_filename, "test.txt");
        assert_eq!(metadata.size, 13);
        
        let retrieved_metadata = manager.get_file_metadata(metadata.id).await.unwrap().unwrap();
        assert_eq!(retrieved_metadata.id, metadata.id);
        
        let (file_metadata, data) = manager.get_file_data(metadata.id).await.unwrap().unwrap();
        assert_eq!(file_metadata.id, metadata.id);
        assert_eq!(data, b"Hello, World!");
    }
    
    #[tokio::test]
    async fn test_file_validation() {
        let (manager, _temp_dir) = create_test_setup().await;
        
        let large_upload = FileUpload {
            original_filename: "large.txt".to_string(),
            content_type: "text/plain".to_string(),
            data: vec![0; 20 * 1024 * 1024],
            uploaded_by: 1,
            item_id: None,
        };
        
        let result = manager.store_file(large_upload).await;
        assert!(result.is_err());
        
        let invalid_upload = FileUpload {
            original_filename: "script.exe".to_string(),
            content_type: "application/x-executable".to_string(),
            data: b"fake executable".to_vec(),
            uploaded_by: 1,
            item_id: None,
        };
        
        let result = manager.store_file(invalid_upload).await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_delete_file() {
        let (manager, _temp_dir) = create_test_setup().await;
        
        let upload = FileUpload {
            original_filename: "delete_me.txt".to_string(),
            content_type: "text/plain".to_string(),
            data: b"Delete this file".to_vec(),
            uploaded_by: 1,
            item_id: None,
        };
        
        let metadata = manager.store_file(upload).await.unwrap();
        
        assert!(manager.get_file_metadata(metadata.id).await.unwrap().is_some());
        
        manager.delete_file(metadata.id).await.unwrap();
        
        assert!(manager.get_file_metadata(metadata.id).await.unwrap().is_none());
    }
}