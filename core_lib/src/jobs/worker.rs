use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{info, error, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};
use crate::websocket::{WebSocketManager, WebSocketEvent};
use crate::jobs::JobResponse;
use super::models::{Job, JobType};
use super::repository::JobRepositoryTrait;

pub struct WorkerPool {
    job_sender: mpsc::UnboundedSender<Job>,
    worker_count: usize,
    _semaphore: Arc<Semaphore>,
}

impl WorkerPool {
    pub async fn new(
        worker_count: usize,
        repository: Arc<dyn JobRepositoryTrait>,
    ) -> Result<Self> {
        Self::new_with_websocket(worker_count, repository, None).await
    }

    pub async fn new_with_websocket(
        worker_count: usize,
        repository: Arc<dyn JobRepositoryTrait>,
        websocket_manager: Option<Arc<WebSocketManager>>,
    ) -> Result<Self> {
        let (job_sender, job_receiver) = mpsc::unbounded_channel();
        let semaphore = Arc::new(Semaphore::new(worker_count));

        // Create a shared receiver using Arc<Mutex<>>
        let shared_receiver = Arc::new(tokio::sync::Mutex::new(job_receiver));

        // Start worker tasks
        for worker_id in 0..worker_count {
            let worker = JobWorker::new(
                worker_id,
                shared_receiver.clone(),
                repository.clone(),
                semaphore.clone(),
                websocket_manager.clone(),
            );
            
            tokio::spawn(async move {
                worker.run().await;
            });
        }

        info!("Started {} job workers", worker_count);

        Ok(Self {
            job_sender,
            worker_count,
            _semaphore: semaphore,
        })
    }

    pub async fn submit_job(&self, job: Job) -> Result<()> {
        self.job_sender.send(job)
            .map_err(|_| AppError::Job("Failed to submit job to worker pool".to_string()))?;
        Ok(())
    }

    pub fn worker_count(&self) -> usize {
        self.worker_count
    }
}

pub struct JobWorker {
    id: usize,
    job_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Job>>>,
    repository: Arc<dyn JobRepositoryTrait>,
    semaphore: Arc<Semaphore>,
    websocket_manager: Option<Arc<WebSocketManager>>,
}

impl JobWorker {
    pub fn new(
        id: usize,
        job_receiver: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Job>>>,
        repository: Arc<dyn JobRepositoryTrait>,
        semaphore: Arc<Semaphore>,
        websocket_manager: Option<Arc<WebSocketManager>>,
    ) -> Self {
        Self {
            id,
            job_receiver,
            repository,
            semaphore,
            websocket_manager,
        }
    }

    pub async fn run(self) {
        info!("Worker {} started", self.id);

        loop {
            // Get job from shared receiver
            let job = {
                let mut receiver = self.job_receiver.lock().await;
                receiver.recv().await
            };

            match job {
                Some(job) => {
                    // Acquire semaphore permit to limit concurrent jobs
                    let _permit = match self.semaphore.acquire().await {
                        Ok(permit) => permit,
                        Err(_) => {
                            error!("Worker {} failed to acquire semaphore permit", self.id);
                            continue;
                        }
                    };

                    if let Err(e) = self.process_job(job).await {
                        error!("Worker {} failed to process job: {}", self.id, e);
                    }
                }
                None => {
                    warn!("Worker {} stopped - channel closed", self.id);
                    break;
                }
            }
        }
    }

    async fn process_job(&self, mut job: Job) -> Result<()> {
        info!("Worker {} processing job {} (type: {:?})", self.id, job.id, job.job_type);

        // Mark job as running
        job.start();
        let updated_job = self.repository.update(&job).await?;
        
        // Send WebSocket notification for job started
        if let Some(ws_manager) = &self.websocket_manager {
            let event = WebSocketEvent::JobStarted(JobResponse::from(updated_job.clone()));
            ws_manager.broadcast(event).await;
        }

        // Process the job based on its type
        let result = self.execute_job(&job).await;

        // Update job status based on result
        match result {
            Ok(job_result) => {
                job.complete(job_result);
                let completed_job = self.repository.update(&job).await?;
                info!("Worker {} completed job {}", self.id, job.id);
                
                // Send WebSocket notification for job completed
                if let Some(ws_manager) = &self.websocket_manager {
                    let event = WebSocketEvent::JobCompleted(JobResponse::from(completed_job));
                    ws_manager.broadcast(event).await;
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                job.fail(error_msg);
                let failed_job = self.repository.update(&job).await?;
                error!("Worker {} failed job {}: {}", self.id, job.id, e);
                
                // Send WebSocket notification for job failed
                if let Some(ws_manager) = &self.websocket_manager {
                    let event = WebSocketEvent::JobFailed(JobResponse::from(failed_job));
                    ws_manager.broadcast(event).await;
                }
            }
        }

        Ok(())
    }

    async fn execute_job(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        match job.job_type {
            JobType::BulkImport => self.execute_bulk_import(job).await,
            JobType::BulkExport => self.execute_bulk_export(job).await,
            JobType::DataMigration => self.execute_data_migration(job).await,
            JobType::FileProcessing => self.execute_file_processing(job).await,
            JobType::EmailNotification => self.execute_email_notification(job).await,
            JobType::ReportGeneration => self.execute_report_generation(job).await,
        }
    }

    async fn execute_bulk_import(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        // Simulate bulk import processing
        info!("Executing bulk import for job {}", job.id);
        
        // Extract import parameters from payload
        let import_data = job.payload.get("data")
            .ok_or_else(|| AppError::Job("Missing import data in payload".to_string()))?;
        
        let items = import_data.as_array()
            .ok_or_else(|| AppError::Job("Import data must be an array".to_string()))?;

        // Simulate processing time
        tokio::time::sleep(tokio::time::Duration::from_millis(100 * items.len() as u64)).await;

        let result = serde_json::json!({
            "imported_count": items.len(),
            "success": true,
            "message": format!("Successfully imported {} items", items.len())
        });

        Ok(Some(result))
    }

    async fn execute_bulk_export(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        info!("Executing bulk export for job {}", job.id);
        
        let format = job.payload.get("format")
            .and_then(|f| f.as_str())
            .unwrap_or("json");
        
        let filters = job.payload.get("filters");

        // Simulate export processing
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let result = serde_json::json!({
            "export_format": format,
            "filters_applied": filters.is_some(),
            "exported_count": 100, // Simulated count
            "file_path": format!("/exports/export_{}.{}", job.id, format),
            "success": true
        });

        Ok(Some(result))
    }

    async fn execute_data_migration(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        info!("Executing data migration for job {}", job.id);
        
        let migration_type = job.payload.get("migration_type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| AppError::Job("Missing migration_type in payload".to_string()))?;

        // Simulate migration processing
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        let result = serde_json::json!({
            "migration_type": migration_type,
            "migrated_records": 250,
            "success": true,
            "duration_seconds": 5
        });

        Ok(Some(result))
    }

    async fn execute_file_processing(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        info!("Executing file processing for job {}", job.id);
        
        let file_id = job.payload.get("file_id")
            .and_then(|f| f.as_str())
            .ok_or_else(|| AppError::Job("Missing file_id in payload".to_string()))?;

        let operation = job.payload.get("operation")
            .and_then(|o| o.as_str())
            .unwrap_or("process");

        // Simulate file processing
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let result = serde_json::json!({
            "file_id": file_id,
            "operation": operation,
            "processed": true,
            "output_file": format!("processed_{}", file_id)
        });

        Ok(Some(result))
    }

    async fn execute_email_notification(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        info!("Executing email notification for job {}", job.id);
        
        let recipient = job.payload.get("recipient")
            .and_then(|r| r.as_str())
            .ok_or_else(|| AppError::Job("Missing recipient in payload".to_string()))?;

        let subject = job.payload.get("subject")
            .and_then(|s| s.as_str())
            .unwrap_or("Notification");

        // Simulate email sending
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let result = serde_json::json!({
            "recipient": recipient,
            "subject": subject,
            "sent": true,
            "message_id": format!("msg_{}", Uuid::new_v4())
        });

        Ok(Some(result))
    }

    async fn execute_report_generation(&self, job: &Job) -> Result<Option<serde_json::Value>> {
        info!("Executing report generation for job {}", job.id);
        
        let report_type = job.payload.get("report_type")
            .and_then(|r| r.as_str())
            .ok_or_else(|| AppError::Job("Missing report_type in payload".to_string()))?;

        let date_range = job.payload.get("date_range");

        // Simulate report generation
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let result = serde_json::json!({
            "report_type": report_type,
            "date_range": date_range,
            "generated": true,
            "report_file": format!("reports/{}_{}.pdf", report_type, job.id),
            "pages": 15
        });

        Ok(Some(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{models::*, repository::JobRepository};
    use serde_json::json;


    async fn create_test_repository() -> Arc<dyn JobRepositoryTrait> {
        let database_url = "sqlite::memory:";
        
        let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        let repo = JobRepository::new(pool);
        repo.create_table().await.unwrap();
        Arc::new(repo)
    }

    #[tokio::test]
    async fn test_worker_pool_creation() {
        let repo = create_test_repository().await;
        let pool = WorkerPool::new(2, repo).await.unwrap();
        assert_eq!(pool.worker_count(), 2);
    }

    #[tokio::test]
    async fn test_bulk_import_job() {
        let repo = create_test_repository().await;
        let pool = WorkerPool::new(1, repo.clone()).await.unwrap();

        let job_request = JobRequest {
            job_type: JobType::BulkImport,
            payload: json!({
                "data": [
                    {"name": "Item 1", "description": "Test item 1"},
                    {"name": "Item 2", "description": "Test item 2"}
                ]
            }),
            priority: Some(JobPriority::Normal),
            max_retries: Some(3),
        };

        let job = Job::new(job_request);
        let job_id = job.id;
        
        // Save job to repository
        repo.create(&job).await.unwrap();
        
        // Submit to worker pool
        pool.submit_job(job).await.unwrap();

        // Wait for processing
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Check job status
        let updated_job = repo.get_by_id(job_id).await.unwrap().unwrap();
        assert_eq!(updated_job.status, JobStatus::Completed);
        assert!(updated_job.result.is_some());
        
        let result = updated_job.result.unwrap();
        assert_eq!(result["imported_count"], 2);
        assert_eq!(result["success"], true);
    }
}