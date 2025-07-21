use crate::{
    error::{AppError, Result},
    jobs::{JobRequest, JobListParams},
    models::request::ApiResponse,
    AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct JobQueryParams {
    pub status: Option<String>,
    pub job_type: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

pub async fn submit_job(
    State(state): State<AppState>,
    Json(request): Json<JobRequest>,
) -> Result<impl IntoResponse> {
    info!("POST /api/jobs - submitting job: {:?}", request.job_type);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let job_id = job_queue.submit_job(request).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(serde_json::json!({
            "job_id": job_id,
            "message": "Job submitted successfully",
            "status": "pending"
        }))),
    ))
}

pub async fn get_job_status(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    info!("GET /api/jobs/{}/status", job_id);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let job = job_queue
        .get_job_status(job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Job not found".to_string()))?;

    Ok(Json(ApiResponse::success(job)))
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    info!("GET /api/jobs/{}", job_id);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let job = job_queue
        .get_job_status(job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Job not found".to_string()))?;

    Ok(Json(ApiResponse::success(job)))
}

pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<JobQueryParams>,
) -> Result<impl IntoResponse> {
    info!("GET /api/jobs - params: {:?}", params);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let status = if let Some(status_str) = params.status {
        Some(parse_job_status(&status_str)?)
    } else {
        None
    };

    let job_type = if let Some(type_str) = params.job_type {
        Some(parse_job_type(&type_str)?)
    } else {
        None
    };

    let list_params = JobListParams {
        status,
        job_type,
        limit: params.limit,
        offset: params.offset,
        sort_by: params.sort_by,
        sort_order: params.sort_order,
    };

    let job_list = job_queue.list_jobs(list_params).await?;

    Ok(Json(ApiResponse::success(job_list)))
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    info!("DELETE /api/jobs/{}/cancel", job_id);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let cancelled = job_queue.cancel_job(job_id).await?;

    if cancelled {
        Ok(Json(ApiResponse::success(serde_json::json!({
            "job_id": job_id,
            "message": "Job cancelled successfully",
            "cancelled": true
        }))))
    } else {
        Err(AppError::BadRequest(
            "Job cannot be cancelled (already running or completed)".to_string(),
        ))
    }
}

pub async fn retry_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    info!("POST /api/jobs/{}/retry", job_id);

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let retried = job_queue.retry_job(job_id).await?;

    if retried {
        Ok(Json(ApiResponse::success(serde_json::json!({
            "job_id": job_id,
            "message": "Job queued for retry",
            "retried": true
        }))))
    } else {
        Err(AppError::BadRequest(
            "Job cannot be retried (not failed or retry limit exceeded)".to_string(),
        ))
    }
}

pub async fn get_queue_stats(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("GET /api/jobs/stats");

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let stats = job_queue.get_queue_stats().await?;

    Ok(Json(ApiResponse::success(stats)))
}

pub async fn cleanup_jobs(State(state): State<AppState>) -> Result<impl IntoResponse> {
    info!("POST /api/jobs/cleanup");

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let deleted_count = job_queue.cleanup_old_jobs(30).await?;

    Ok(Json(ApiResponse::success(serde_json::json!({
        "message": "Job cleanup completed",
        "deleted_count": deleted_count
    }))))
}

pub async fn submit_bulk_import(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse> {
    info!("POST /api/jobs/bulk-import");

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let request = JobRequest {
        job_type: crate::jobs::JobType::BulkImport,
        payload,
        priority: Some(crate::jobs::JobPriority::High),
        max_retries: Some(3),
    };

    let job_id = job_queue.submit_job(request).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(serde_json::json!({
            "job_id": job_id,
            "job_type": "bulk_import",
            "message": "Bulk import job submitted successfully",
            "status": "pending"
        }))),
    ))
}

pub async fn submit_bulk_export(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse> {
    info!("POST /api/jobs/bulk-export");

    let job_queue = state
        .job_queue
        .as_ref()
        .ok_or_else(|| AppError::Job("Job queue not available".to_string()))?;

    let request = JobRequest {
        job_type: crate::jobs::JobType::BulkExport,
        payload,
        priority: Some(crate::jobs::JobPriority::Normal),
        max_retries: Some(2),
    };

    let job_id = job_queue.submit_job(request).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(serde_json::json!({
            "job_id": job_id,
            "job_type": "bulk_export",
            "message": "Bulk export job submitted successfully",
            "status": "pending"
        }))),
    ))
}

fn parse_job_status(status_str: &str) -> Result<crate::jobs::JobStatus> {
    match status_str.to_lowercase().as_str() {
        "pending" => Ok(crate::jobs::JobStatus::Pending),
        "running" => Ok(crate::jobs::JobStatus::Running),
        "completed" => Ok(crate::jobs::JobStatus::Completed),
        "failed" => Ok(crate::jobs::JobStatus::Failed),
        "cancelled" => Ok(crate::jobs::JobStatus::Cancelled),
        "retrying" => Ok(crate::jobs::JobStatus::Retrying),
        _ => Err(AppError::BadRequest(format!(
            "Invalid job status: {}. Valid values: pending, running, completed, failed, cancelled, retrying",
            status_str
        ))),
    }
}

fn parse_job_type(type_str: &str) -> Result<crate::jobs::JobType> {
    match type_str.to_lowercase().as_str() {
        "bulk_import" | "bulkimport" => Ok(crate::jobs::JobType::BulkImport),
        "bulk_export" | "bulkexport" => Ok(crate::jobs::JobType::BulkExport),
        "data_migration" | "datamigration" => Ok(crate::jobs::JobType::DataMigration),
        "file_processing" | "fileprocessing" => Ok(crate::jobs::JobType::FileProcessing),
        "email_notification" | "emailnotification" => Ok(crate::jobs::JobType::EmailNotification),
        "report_generation" | "reportgeneration" => Ok(crate::jobs::JobType::ReportGeneration),
        _ => Err(AppError::BadRequest(format!(
            "Invalid job type: {}. Valid values: bulk_import, bulk_export, data_migration, file_processing, email_notification, report_generation",
            type_str
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{JobQueue, JobRepository};
    use serde_json::json;

    async fn create_test_app_state() -> AppState {
        let database_url = "sqlite::memory:";
        let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        let job_repo = JobRepository::new(pool);
        job_repo.create_table().await.unwrap();

        let job_queue = JobQueue::new(job_repo);
        job_queue.start_workers(2).await.unwrap();

        AppState::default().with_job_queue(job_queue)
    }

    #[tokio::test]
    async fn test_submit_job() {
        let state = create_test_app_state().await;

        let request = JobRequest {
            job_type: crate::jobs::JobType::BulkImport,
            payload: json!({"data": [{"name": "test"}]}),
            priority: Some(crate::jobs::JobPriority::High),
            max_retries: Some(3),
        };

        let _response = submit_job(State(state), Json(request)).await.unwrap();
    }

    #[tokio::test]
    async fn test_parse_job_status() {
        assert!(matches!(
            parse_job_status("pending").unwrap(),
            crate::jobs::JobStatus::Pending
        ));
        assert!(matches!(
            parse_job_status("RUNNING").unwrap(),
            crate::jobs::JobStatus::Running
        ));
        assert!(parse_job_status("invalid").is_err());
    }

    #[tokio::test]
    async fn test_parse_job_type() {
        assert!(matches!(
            parse_job_type("bulk_import").unwrap(),
            crate::jobs::JobType::BulkImport
        ));
        assert!(matches!(
            parse_job_type("BULKEXPORT").unwrap(),
            crate::jobs::JobType::BulkExport
        ));
        assert!(parse_job_type("invalid").is_err());
    }
}