use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct File {
    pub id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: u64,
    pub path: String,
    pub uploaded_by: u64,
    pub created_at: DateTime<Utc>,
    pub item_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: Uuid,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: u64,
    pub uploaded_by: u64,
    pub created_at: DateTime<Utc>,
    pub item_id: Option<u64>,
}

impl From<File> for FileMetadata {
    fn from(file: File) -> Self {
        Self {
            id: file.id,
            filename: file.filename,
            original_filename: file.original_filename,
            content_type: file.content_type,
            size: file.size,
            uploaded_by: file.uploaded_by,
            created_at: file.created_at,
            item_id: file.item_id,
        }
    }
}

#[derive(Debug)]
pub struct FileUpload {
    pub original_filename: String,
    pub content_type: String,
    pub data: Vec<u8>,
    pub uploaded_by: u64,
    pub item_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListQuery {
    pub item_id: Option<u64>,
    pub content_type: Option<String>,
    pub uploaded_by: Option<u64>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

impl Default for FileListQuery {
    fn default() -> Self {
        Self {
            item_id: None,
            content_type: None,
            uploaded_by: None,
            limit: Some(50),
            offset: Some(0),
        }
    }
}