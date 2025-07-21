use crate::{
    jobs::{JobQueue, JobRepository, JobRequest, JobType, JobPriority},
    websocket::WebSocketManager,
};
use serde_json::json;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_job_system_with_websocket_integration() {
    let database_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
    let job_repo = JobRepository::new(pool);
    job_repo.create_table().await.unwrap();

    let ws_manager = WebSocketManager::new(None);
    let ws_manager_arc = Arc::new(ws_manager);

    let job_queue = JobQueue::new_with_websocket(job_repo, Some(ws_manager_arc.clone()));
    
    job_queue.start_workers(2).await.unwrap();

    let request = JobRequest {
        job_type: JobType::BulkImport,
        payload: json!({
            "data": [
                {"name": "Item 1", "description": "Test item 1"},
                {"name": "Item 2", "description": "Test item 2"},
                {"name": "Item 3", "description": "Test item 3"}
            ]
        }),
        priority: Some(JobPriority::High),
        max_retries: Some(3),
    };

    let job_id = job_queue.submit_job(request).await.unwrap();
    println!("Submitted job with ID: {}", job_id);

    sleep(Duration::from_secs(2)).await;

    let job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    println!("Job status: {:?}", job.status);
    println!("Job result: {:?}", job.result);

    assert_eq!(job.status, crate::jobs::JobStatus::Completed);
    assert!(job.result.is_some());
    
    let result = job.result.unwrap();
    assert_eq!(result["imported_count"], 3);
    assert_eq!(result["success"], true);

    let export_request = JobRequest {
        job_type: JobType::BulkExport,
        payload: json!({
            "format": "json",
            "filters": {
                "created_after": "2024-01-01T00:00:00Z"
            }
        }),
        priority: Some(JobPriority::Normal),
        max_retries: Some(2),
    };

    let export_job_id = job_queue.submit_job(export_request).await.unwrap();
    println!("Submitted export job with ID: {}", export_job_id);

    sleep(Duration::from_secs(3)).await;

    let export_job = job_queue.get_job_status(export_job_id).await.unwrap().unwrap();
    println!("Export job status: {:?}", export_job.status);
    println!("Export job result: {:?}", export_job.result);

    assert_eq!(export_job.status, crate::jobs::JobStatus::Completed);
    assert!(export_job.result.is_some());

    let export_result = export_job.result.unwrap();
    assert_eq!(export_result["export_format"], "json");
    assert_eq!(export_result["success"], true);

    let stats = job_queue.get_queue_stats().await.unwrap();
    println!("Queue stats: {:?}", stats);
    
    assert_eq!(stats.completed_jobs, 2);
    assert_eq!(stats.failed_jobs, 0);
    assert_eq!(stats.active_workers, 2);
}

#[tokio::test]
async fn test_job_retry_mechanism() {
    let database_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
    let job_repo = JobRepository::new(pool);
    job_repo.create_table().await.unwrap();

    let job_queue = JobQueue::new(job_repo);
    job_queue.start_workers(1).await.unwrap();

    let request = JobRequest {
        job_type: JobType::EmailNotification,
        payload: json!({
            "subject": "Test Email"
        }),
        priority: Some(JobPriority::Normal),
        max_retries: Some(2),
    };

    let job_id = job_queue.submit_job(request).await.unwrap();
    
    sleep(Duration::from_secs(2)).await;

    let job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, crate::jobs::JobStatus::Failed);
    assert!(job.error_message.is_some());
    assert_eq!(job.retry_count, 0);

    let retried = job_queue.retry_job(job_id).await.unwrap();
    assert!(retried);

    sleep(Duration::from_secs(2)).await;

    let retried_job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    assert_eq!(retried_job.status, crate::jobs::JobStatus::Failed);
    assert_eq!(retried_job.retry_count, 1);

    let retried_again = job_queue.retry_job(job_id).await.unwrap();
    assert!(retried_again);

    sleep(Duration::from_secs(2)).await;

    let final_job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    assert_eq!(final_job.status, crate::jobs::JobStatus::Failed);
    assert_eq!(final_job.retry_count, 2);

    let cannot_retry = job_queue.retry_job(job_id).await.unwrap();
    assert!(!cannot_retry);
}

#[tokio::test]
async fn test_job_cancellation() {
    let database_url = "sqlite::memory:";
    let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
    let job_repo = JobRepository::new(pool);
    job_repo.create_table().await.unwrap();

    let job_queue = JobQueue::new(job_repo);

    let request = JobRequest {
        job_type: JobType::ReportGeneration,
        payload: json!({
            "report_type": "monthly_summary",
            "date_range": {
                "start": "2024-01-01",
                "end": "2024-01-31"
            }
        }),
        priority: Some(JobPriority::Low),
        max_retries: Some(1),
    };

    let job_id = job_queue.submit_job(request).await.unwrap();

    let job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    assert_eq!(job.status, crate::jobs::JobStatus::Pending);

    let cancelled = job_queue.cancel_job(job_id).await.unwrap();
    assert!(cancelled);

    let cancelled_job = job_queue.get_job_status(job_id).await.unwrap().unwrap();
    assert_eq!(cancelled_job.status, crate::jobs::JobStatus::Cancelled);

    let cannot_cancel = job_queue.cancel_job(job_id).await.unwrap();
    assert!(!cannot_cancel);
}