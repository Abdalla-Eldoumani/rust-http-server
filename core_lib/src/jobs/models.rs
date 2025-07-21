use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub job_type: JobType,
    pub status: JobStatus,
    pub payload: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub priority: JobPriority,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_type", rename_all = "snake_case")]
pub enum JobType {
    BulkImport,
    BulkExport,
    DataMigration,
    FileProcessing,
    EmailNotification,
    ReportGeneration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_status", rename_all = "snake_case")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "job_priority", rename_all = "snake_case")]
pub enum JobPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl Default for JobPriority {
    fn default() -> Self {
        JobPriority::Normal
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    pub job_type: JobType,
    pub payload: serde_json::Value,
    pub priority: Option<JobPriority>,
    pub max_retries: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub job_type: JobType,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub priority: JobPriority,
}

impl From<Job> for JobResponse {
    fn from(job: Job) -> Self {
        Self {
            id: job.id,
            job_type: job.job_type,
            status: job.status,
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
            result: job.result,
            error_message: job.error_message,
            retry_count: job.retry_count,
            max_retries: job.max_retries,
            priority: job.priority,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListParams {
    pub status: Option<JobStatus>,
    pub job_type: Option<JobType>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

impl Default for JobListParams {
    fn default() -> Self {
        Self {
            status: None,
            job_type: None,
            limit: Some(50),
            offset: Some(0),
            sort_by: Some("created_at".to_string()),
            sort_order: Some("desc".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobResponse>,
    pub total: u64,
    pub limit: u32,
    pub offset: u32,
}

impl Job {
    pub fn new(request: JobRequest) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            job_type: request.job_type,
            status: JobStatus::Pending,
            payload: request.payload,
            result: None,
            error_message: None,
            created_at: now,
            started_at: None,
            completed_at: None,
            retry_count: 0,
            max_retries: request.max_retries.unwrap_or(3),
            priority: request.priority.unwrap_or_default(),
        }
    }

    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
    }

    pub fn complete(&mut self, result: Option<serde_json::Value>) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.result = result;
    }

    pub fn fail(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error_message = Some(error);
    }

    pub fn cancel(&mut self) {
        self.status = JobStatus::Cancelled;
        self.completed_at = Some(Utc::now());
    }

    pub fn retry(&mut self) {
        self.retry_count += 1;
        self.status = if self.retry_count >= self.max_retries {
            JobStatus::Failed
        } else {
            JobStatus::Retrying
        };
        self.started_at = None;
        self.completed_at = None;
    }

    pub fn can_retry(&self) -> bool {
        matches!(self.status, JobStatus::Failed | JobStatus::Retrying) 
            && self.retry_count < self.max_retries
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.status, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled)
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, JobStatus::Running)
    }
}

impl JobPriority {
    pub fn to_numeric(&self) -> i32 {
        match self {
            JobPriority::Critical => 4,
            JobPriority::High => 3,
            JobPriority::Normal => 2,
            JobPriority::Low => 1,
        }
    }
}