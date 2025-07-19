use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbItem {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub tags: String,
    pub metadata: String,
    pub created_by: Option<i64>,
}

impl DbItem {
    pub fn to_api_item(&self) -> crate::store::Item {
        crate::store::Item {
            id: self.id as u64,
            name: self.name.clone(),
            description: self.description.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            tags: serde_json::from_str(&self.tags).unwrap_or_default(),
            metadata: serde_json::from_str(&self.metadata).ok(),
        }
    }

    pub fn from_api_item(item: &crate::store::Item, created_by: Option<i64>) -> Self {
        Self {
            id: item.id as i64,
            name: item.name.clone(),
            description: item.description.clone(),
            created_at: item.created_at,
            updated_at: item.updated_at,
            tags: serde_json::to_string(&item.tags).unwrap_or_else(|_| "[]".to_string()),
            metadata: item.metadata.as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                .unwrap_or_else(|| "{}".to_string()),
            created_by,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbUser {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserRole {
    Admin,
    User,
    ReadOnly,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::Admin => write!(f, "Admin"),
            UserRole::User => write!(f, "User"),
            UserRole::ReadOnly => write!(f, "ReadOnly"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Admin" => Ok(UserRole::Admin),
            "User" => Ok(UserRole::User),
            "ReadOnly" => Ok(UserRole::ReadOnly),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbFile {
    pub id: String,
    pub filename: String,
    pub original_filename: String,
    pub content_type: String,
    pub size: i64,
    pub path: String,
    pub uploaded_by: i64,
    pub created_at: DateTime<Utc>,
    pub item_id: Option<i64>,
}

impl DbFile {
    pub fn uuid(&self) -> Result<Uuid, uuid::Error> {
        Uuid::parse_str(&self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbJob {
    pub id: String,
    pub job_type: String,
    pub payload: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_by: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Pending => write!(f, "Pending"),
            JobStatus::Running => write!(f, "Running"),
            JobStatus::Completed => write!(f, "Completed"),
            JobStatus::Failed => write!(f, "Failed"),
            JobStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(JobStatus::Pending),
            "Running" => Ok(JobStatus::Running),
            "Completed" => Ok(JobStatus::Completed),
            "Failed" => Ok(JobStatus::Failed),
            "Cancelled" => Ok(JobStatus::Cancelled),
            _ => Err(format!("Invalid job status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobType {
    BulkImport,
    BulkExport,
    DataMigration,
    FileProcessing,
}

impl std::fmt::Display for JobType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobType::BulkImport => write!(f, "BulkImport"),
            JobType::BulkExport => write!(f, "BulkExport"),
            JobType::DataMigration => write!(f, "DataMigration"),
            JobType::FileProcessing => write!(f, "FileProcessing"),
        }
    }
}

impl std::str::FromStr for JobType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "BulkImport" => Ok(JobType::BulkImport),
            "BulkExport" => Ok(JobType::BulkExport),
            "DataMigration" => Ok(JobType::DataMigration),
            "FileProcessing" => Ok(JobType::FileProcessing),
            _ => Err(format!("Invalid job type: {}", s)),
        }
    }
}

pub struct Transaction<'a> {
    tx: sqlx::Transaction<'a, sqlx::Sqlite>,
}

impl<'a> Transaction<'a> {
    pub fn new(tx: sqlx::Transaction<'a, sqlx::Sqlite>) -> Self {
        Self { tx }
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await
    }

    pub async fn rollback(self) -> Result<(), sqlx::Error> {
        self.tx.rollback().await
    }

    pub fn as_mut(&mut self) -> &mut sqlx::Transaction<'a, sqlx::Sqlite> {
        &mut self.tx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_conversion() {
        assert_eq!(UserRole::Admin.to_string(), "Admin");
        assert_eq!("User".parse::<UserRole>().unwrap(), UserRole::User);
        assert!("InvalidRole".parse::<UserRole>().is_err());
    }

    #[test]
    fn test_job_status_conversion() {
        assert_eq!(JobStatus::Pending.to_string(), "Pending");
        assert_eq!("Running".parse::<JobStatus>().unwrap(), JobStatus::Running);
        assert!("InvalidStatus".parse::<JobStatus>().is_err());
    }

    #[test]
    fn test_job_type_conversion() {
        assert_eq!(JobType::BulkImport.to_string(), "BulkImport");
        assert_eq!("BulkExport".parse::<JobType>().unwrap(), JobType::BulkExport);
        assert!("InvalidType".parse::<JobType>().is_err());
    }

    #[test]
    fn test_db_item_conversion() {
        let api_item = crate::store::Item {
            id: 1,
            name: "Test Item".to_string(),
            description: Some("Test Description".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            metadata: Some(serde_json::json!({"key": "value"})),
        };

        let db_item = DbItem::from_api_item(&api_item, Some(1));
        assert_eq!(db_item.name, api_item.name);
        assert_eq!(db_item.description, api_item.description);
        assert_eq!(db_item.created_by, Some(1));

        let converted_back = db_item.to_api_item();
        assert_eq!(converted_back.name, api_item.name);
        assert_eq!(converted_back.description, api_item.description);
        assert_eq!(converted_back.tags, api_item.tags);
    }
}