use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, error, warn};
use uuid::Uuid;

use crate::error::{AppError, Result};
use super::models::{Job, JobRequest, JobStatus};
use super::repository::{JobRepository, JobRepositoryTrait};
use super::worker::WorkerPool;

#[derive(Clone)]
pub struct JobQueue {
    sender: mpsc::UnboundedSender<Job>,
    repository: Arc<dyn JobRepositoryTrait>,
    worker_pool: Arc<RwLock<Option<WorkerPool>>>,
    websocket_manager: Option<Arc<crate::websocket::WebSocketManager>>,
}

impl JobQueue {
    pub fn new(repository: JobRepository) -> Self {
        Self::new_with_websocket(repository, None)
    }

    pub fn new_with_websocket(
        repository: JobRepository, 
        websocket_manager: Option<Arc<crate::websocket::WebSocketManager>>
    ) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let repository = Arc::new(repository);
        
        let queue = Self {
            sender,
            repository: repository.clone(),
            worker_pool: Arc::new(RwLock::new(None)),
            websocket_manager,
        };

        let queue_clone = queue.clone();
        tokio::spawn(async move {
            queue_clone.process_queue(receiver).await;
        });

        queue
    }

    pub async fn start_workers(&self, worker_count: usize) -> Result<()> {
        let worker_pool = WorkerPool::new_with_websocket(
            worker_count, 
            self.repository.clone(),
            self.websocket_manager.clone()
        ).await?;
        
        self.process_pending_jobs().await?;
        
        *self.worker_pool.write().await = Some(worker_pool);
        
        info!("Started job queue with {} workers", worker_count);
        Ok(())
    }

    pub async fn submit_job(&self, request: JobRequest) -> Result<Uuid> {
        let mut job = Job::new(request);
        
        job = self.repository.create(&job).await?;
        
        self.sender.send(job.clone())
            .map_err(|_| AppError::Job("Failed to queue job".to_string()))?;
        
        info!("Job {} submitted for processing", job.id);
        Ok(job.id)
    }

    pub async fn get_job_status(&self, job_id: Uuid) -> Result<Option<Job>> {
        self.repository.get_by_id(job_id).await
    }

    pub async fn cancel_job(&self, job_id: Uuid) -> Result<bool> {
        if let Some(mut job) = self.repository.get_by_id(job_id).await? {
            if !job.is_terminal() && !job.is_running() {
                job.cancel();
                let cancelled_job = self.repository.update(&job).await?;
                info!("Job {} cancelled", job_id);
                
                if let Some(ws_manager) = &self.websocket_manager {
                    let event = crate::websocket::WebSocketEvent::JobCancelled(
                        crate::jobs::JobResponse::from(cancelled_job)
                    );
                    ws_manager.broadcast(event).await;
                }
                
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn retry_job(&self, job_id: Uuid) -> Result<bool> {
        if let Some(mut job) = self.repository.get_by_id(job_id).await? {
            if job.can_retry() {
                job.retry();
                job = self.repository.update(&job).await?;
                
                if let Some(ws_manager) = &self.websocket_manager {
                    let event = crate::websocket::WebSocketEvent::JobRetrying(
                        crate::jobs::JobResponse::from(job.clone())
                    );
                    ws_manager.broadcast(event).await;
                }
                
                self.sender.send(job.clone())
                    .map_err(|_| AppError::Job("Failed to re-queue job".to_string()))?;
                
                info!("Job {} queued for retry (attempt {})", job_id, job.retry_count + 1);
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn get_queue_stats(&self) -> Result<QueueStats> {
        let pending_jobs = self.repository.get_jobs_by_status(JobStatus::Pending).await?;
        let running_jobs = self.repository.get_jobs_by_status(JobStatus::Running).await?;
        let completed_jobs = self.repository.get_jobs_by_status(JobStatus::Completed).await?;
        let failed_jobs = self.repository.get_jobs_by_status(JobStatus::Failed).await?;

        let worker_count = if let Some(pool) = self.worker_pool.read().await.as_ref() {
            pool.worker_count()
        } else {
            0
        };

        Ok(QueueStats {
            pending_jobs: pending_jobs.len() as u64,
            running_jobs: running_jobs.len() as u64,
            completed_jobs: completed_jobs.len() as u64,
            failed_jobs: failed_jobs.len() as u64,
            active_workers: worker_count,
        })
    }

    pub async fn cleanup_old_jobs(&self, days: u32) -> Result<u64> {
        let deleted_count = self.repository.cleanup_old_jobs(days).await?;
        info!("Cleaned up {} old jobs", deleted_count);
        Ok(deleted_count)
    }

    pub async fn list_jobs(&self, params: crate::jobs::JobListParams) -> Result<crate::jobs::JobListResponse> {
        self.repository.list(params).await
    }

    async fn process_queue(&self, mut receiver: mpsc::UnboundedReceiver<Job>) {
        info!("Job queue processor started");
        
        while let Some(job) = receiver.recv().await {
            if let Err(e) = self.process_single_job(job).await {
                error!("Failed to process job: {}", e);
            }
        }
        
        warn!("Job queue processor stopped");
    }

    async fn process_single_job(&self, job: Job) -> Result<()> {
        if job.is_terminal() || job.is_running() {
            return Ok(());
        }

        let worker_pool = self.worker_pool.read().await;
        if let Some(pool) = worker_pool.as_ref() {
            pool.submit_job(job).await?;
        } else {
            warn!("No worker pool available, job {} will remain pending", job.id);
        }

        Ok(())
    }

    async fn process_pending_jobs(&self) -> Result<()> {
        let pending_jobs = self.repository.get_pending_jobs(100).await?;
        let job_count = pending_jobs.len();
        
        for job in pending_jobs {
            self.sender.send(job)
                .map_err(|_| AppError::Job("Failed to queue pending job".to_string()))?;
        }
        
        info!("Queued {} pending jobs for processing", job_count);
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueStats {
    pub pending_jobs: u64,
    pub running_jobs: u64,
    pub completed_jobs: u64,
    pub failed_jobs: u64,
    pub active_workers: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::models::{JobType, JobPriority};
    use serde_json::json;


    async fn create_test_repository() -> JobRepository {
        let database_url = "sqlite::memory:";
        
        let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        let repo = JobRepository::new(pool);
        repo.create_table().await.unwrap();
        repo
    }

    #[tokio::test]
    async fn test_job_queue_creation() {
        let repo = create_test_repository().await;
        let queue = JobQueue::new(repo);
        
        let stats = queue.get_queue_stats().await.unwrap();
        assert_eq!(stats.pending_jobs, 0);
        assert_eq!(stats.active_workers, 0);
    }

    #[tokio::test]
    async fn test_job_submission() {
        let repo = create_test_repository().await;
        let queue = JobQueue::new(repo);
        
        let request = JobRequest {
            job_type: JobType::BulkImport,
            payload: json!({"test": "data"}),
            priority: Some(JobPriority::High),
            max_retries: Some(2),
        };
        
        let job_id = queue.submit_job(request).await.unwrap();
        
        let job = queue.get_job_status(job_id).await.unwrap().unwrap();
        assert_eq!(job.job_type, JobType::BulkImport);
        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.priority, JobPriority::High);
        assert_eq!(job.max_retries, 2);
    }

    #[tokio::test]
    async fn test_job_cancellation() {
        let repo = create_test_repository().await;
        let queue = JobQueue::new(repo);
        
        let request = JobRequest {
            job_type: JobType::BulkExport,
            payload: json!({"test": "data"}),
            priority: None,
            max_retries: None,
        };
        
        let job_id = queue.submit_job(request).await.unwrap();
        let cancelled = queue.cancel_job(job_id).await.unwrap();
        assert!(cancelled);
        
        let job = queue.get_job_status(job_id).await.unwrap().unwrap();
        assert_eq!(job.status, JobStatus::Cancelled);
    }
}