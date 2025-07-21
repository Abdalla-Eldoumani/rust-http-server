use async_trait::async_trait;
use chrono::Utc;
use sqlx::{SqlitePool, Row};
use uuid::Uuid;

use crate::error::{AppError, Result};
use super::models::{Job, JobStatus, JobType, JobPriority, JobListParams, JobListResponse, JobResponse};

#[async_trait]
pub trait JobRepositoryTrait: Send + Sync {
    async fn create(&self, job: &Job) -> Result<Job>;
    async fn get_by_id(&self, id: Uuid) -> Result<Option<Job>>;
    async fn update(&self, job: &Job) -> Result<Job>;
    async fn delete(&self, id: Uuid) -> Result<()>;
    async fn list(&self, params: JobListParams) -> Result<JobListResponse>;
    async fn get_pending_jobs(&self, limit: u32) -> Result<Vec<Job>>;
    async fn get_jobs_by_status(&self, status: JobStatus) -> Result<Vec<Job>>;
    async fn cleanup_old_jobs(&self, days: u32) -> Result<u64>;
}

#[derive(Clone)]
pub struct JobRepository {
    pool: SqlitePool,
}

impl JobRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
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
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for better query performance
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_type ON jobs(job_type)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_jobs_priority ON jobs(priority)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl JobRepositoryTrait for JobRepository {
    async fn create(&self, job: &Job) -> Result<Job> {
        let job_type_str = serde_json::to_string(&job.job_type)?;
        let status_str = serde_json::to_string(&job.status)?;
        let priority_str = serde_json::to_string(&job.priority)?;
        let payload_str = serde_json::to_string(&job.payload)?;
        let result_str = job.result.as_ref().map(|r| serde_json::to_string(r)).transpose()?;

        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, job_type, status, payload, result, error_message,
                created_at, started_at, completed_at, retry_count, max_retries, priority
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(job.id.to_string())
        .bind(job_type_str.trim_matches('"'))
        .bind(status_str.trim_matches('"'))
        .bind(payload_str)
        .bind(result_str)
        .bind(&job.error_message)
        .bind(job.created_at.to_rfc3339())
        .bind(job.started_at.map(|dt| dt.to_rfc3339()))
        .bind(job.completed_at.map(|dt| dt.to_rfc3339()))
        .bind(job.retry_count)
        .bind(job.max_retries)
        .bind(priority_str.trim_matches('"'))
        .execute(&self.pool)
        .await?;

        Ok(job.clone())
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<Job>> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            let job = self.row_to_job(row)?;
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, job: &Job) -> Result<Job> {
        let job_type_str = serde_json::to_string(&job.job_type)?;
        let status_str = serde_json::to_string(&job.status)?;
        let priority_str = serde_json::to_string(&job.priority)?;
        let payload_str = serde_json::to_string(&job.payload)?;
        let result_str = job.result.as_ref().map(|r| serde_json::to_string(r)).transpose()?;

        sqlx::query(
            r#"
            UPDATE jobs SET
                job_type = ?, status = ?, payload = ?, result = ?, error_message = ?,
                started_at = ?, completed_at = ?, retry_count = ?, max_retries = ?, priority = ?
            WHERE id = ?
            "#,
        )
        .bind(job_type_str.trim_matches('"'))
        .bind(status_str.trim_matches('"'))
        .bind(payload_str)
        .bind(result_str)
        .bind(&job.error_message)
        .bind(job.started_at.map(|dt| dt.to_rfc3339()))
        .bind(job.completed_at.map(|dt| dt.to_rfc3339()))
        .bind(job.retry_count)
        .bind(job.max_retries)
        .bind(priority_str.trim_matches('"'))
        .bind(job.id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(job.clone())
    }

    async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM jobs WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn list(&self, params: JobListParams) -> Result<JobListResponse> {
        let mut query = "SELECT * FROM jobs WHERE 1=1".to_string();
        let mut count_query = "SELECT COUNT(*) FROM jobs WHERE 1=1".to_string();
        let mut bind_values = Vec::new();

        if let Some(status) = &params.status {
            let status_str = serde_json::to_string(status)?;
            query.push_str(" AND status = ?");
            count_query.push_str(" AND status = ?");
            bind_values.push(status_str.trim_matches('"').to_string());
        }

        if let Some(job_type) = &params.job_type {
            let type_str = serde_json::to_string(job_type)?;
            query.push_str(" AND job_type = ?");
            count_query.push_str(" AND job_type = ?");
            bind_values.push(type_str.trim_matches('"').to_string());
        }

        // Add sorting
        let sort_by = params.sort_by.as_deref().unwrap_or("created_at");
        let sort_order = params.sort_order.as_deref().unwrap_or("desc");
        query.push_str(&format!(" ORDER BY {} {}", sort_by, sort_order));

        // Add pagination
        let limit = params.limit.unwrap_or(50);
        let offset = params.offset.unwrap_or(0);
        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        // Get total count
        let mut count_query_builder = sqlx::query(&count_query);
        for value in &bind_values {
            count_query_builder = count_query_builder.bind(value);
        }
        let total: i64 = count_query_builder
            .fetch_one(&self.pool)
            .await?
            .get(0);

        // Get jobs
        let mut query_builder = sqlx::query(&query);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }
        let rows = query_builder.fetch_all(&self.pool).await?;

        let jobs: Result<Vec<Job>> = rows.into_iter().map(|row| self.row_to_job(row)).collect();
        let jobs = jobs?;

        Ok(JobListResponse {
            jobs: jobs.into_iter().map(JobResponse::from).collect(),
            total: total as u64,
            limit,
            offset,
        })
    }

    async fn get_pending_jobs(&self, limit: u32) -> Result<Vec<Job>> {
        let rows = sqlx::query(
            "SELECT * FROM jobs WHERE status IN ('pending', 'retrying') ORDER BY priority DESC, created_at ASC LIMIT ?"
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let jobs: Result<Vec<Job>> = rows.into_iter().map(|row| self.row_to_job(row)).collect();
        jobs
    }

    async fn get_jobs_by_status(&self, status: JobStatus) -> Result<Vec<Job>> {
        let status_str = serde_json::to_string(&status)?;
        let rows = sqlx::query("SELECT * FROM jobs WHERE status = ?")
            .bind(status_str.trim_matches('"'))
            .fetch_all(&self.pool)
            .await?;

        let jobs: Result<Vec<Job>> = rows.into_iter().map(|row| self.row_to_job(row)).collect();
        jobs
    }

    async fn cleanup_old_jobs(&self, days: u32) -> Result<u64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);
        let result = sqlx::query(
            "DELETE FROM jobs WHERE completed_at IS NOT NULL AND completed_at < ?"
        )
        .bind(cutoff_date.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

impl JobRepository {
    fn row_to_job(&self, row: sqlx::sqlite::SqliteRow) -> Result<Job> {
        let id_str: String = row.get("id");
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| AppError::Database(format!("Invalid UUID: {}", e)))?;

        let job_type_str: String = row.get("job_type");
        let job_type: JobType = serde_json::from_str(&format!("\"{}\"", job_type_str))?;

        let status_str: String = row.get("status");
        let status: JobStatus = serde_json::from_str(&format!("\"{}\"", status_str))?;

        let priority_str: String = row.get("priority");
        let priority: JobPriority = serde_json::from_str(&format!("\"{}\"", priority_str))?;

        let payload_str: String = row.get("payload");
        let payload: serde_json::Value = serde_json::from_str(&payload_str)?;

        let result: Option<serde_json::Value> = row.get::<Option<String>, _>("result")
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        let created_at_str: String = row.get("created_at");
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| AppError::Database(format!("Invalid datetime: {}", e)))?
            .with_timezone(&Utc);

        let started_at = row.get::<Option<String>, _>("started_at")
            .map(|s| chrono::DateTime::parse_from_rfc3339(&s))
            .transpose()
            .map_err(|e| AppError::Database(format!("Invalid started_at datetime: {}", e)))?
            .map(|dt| dt.with_timezone(&Utc));

        let completed_at = row.get::<Option<String>, _>("completed_at")
            .map(|s| chrono::DateTime::parse_from_rfc3339(&s))
            .transpose()
            .map_err(|e| AppError::Database(format!("Invalid completed_at datetime: {}", e)))?
            .map(|dt| dt.with_timezone(&Utc));

        Ok(Job {
            id,
            job_type,
            status,
            payload,
            result,
            error_message: row.get("error_message"),
            created_at,
            started_at,
            completed_at,
            retry_count: row.get("retry_count"),
            max_retries: row.get("max_retries"),
            priority,
        })
    }
}