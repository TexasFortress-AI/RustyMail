// Copyright (c) 2025 TexasFortress.AI
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;
use sqlx::SqlitePool;
use log::{debug, error, info, warn};
use chrono::{DateTime, Utc};

/// Status of a background job
#[derive(Serialize, Clone)]
#[serde(tag = "status", content = "data")]
pub enum JobStatus {
    Running,
    Completed(Value),
    Failed(String),
}

/// A background job record (in-memory)
#[derive(Clone)]
pub struct JobRecord {
    pub job_id: String,
    pub status: JobStatus,
    pub started_at: Instant,
    pub instruction: Option<String>,
}

// Custom Serialize implementation for JobRecord to control output
impl Serialize for JobRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("JobRecord", 3)?;
        state.serialize_field("job_id", &self.job_id)?;
        state.serialize_field("status", &self.status)?;
        state.serialize_field("instruction", &self.instruction)?;
        state.end()
    }
}

/// Persistent job record stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedJob {
    pub job_id: String,
    pub instruction: Option<String>,
    pub status: String,  // "running", "completed", "failed", "cancelled"
    pub result_data: Option<String>,  // JSON string
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub resumable: bool,
    pub resume_checkpoint: Option<String>,  // JSON checkpoint data
    pub retry_count: i32,
    pub max_retries: i32,
}

impl PersistedJob {
    /// Create a new job record
    pub fn new(job_id: String, instruction: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            job_id,
            instruction,
            status: "running".to_string(),
            result_data: None,
            error_message: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
            resumable: false,
            resume_checkpoint: None,
            retry_count: 0,
            max_retries: 3,
        }
    }

    /// Create a resumable job
    pub fn new_resumable(job_id: String, instruction: Option<String>) -> Self {
        let mut job = Self::new(job_id, instruction);
        job.resumable = true;
        job
    }
}

/// Service for persisting jobs to the database
pub struct JobPersistenceService {
    pool: SqlitePool,
}

impl JobPersistenceService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new job in the database
    pub async fn create_job(&self, job: &PersistedJob) -> Result<(), String> {
        debug!("Creating persisted job: {}", job.job_id);

        sqlx::query(
            r#"
            INSERT INTO background_jobs (job_id, instruction, status, resumable, max_retries)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(&job.job_id)
        .bind(&job.instruction)
        .bind(&job.status)
        .bind(job.resumable)
        .bind(job.max_retries)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            error!("Failed to create job {}: {}", job.job_id, e);
            format!("Database error: {}", e)
        })?;

        debug!("Successfully created job: {}", job.job_id);
        Ok(())
    }

    /// Update job status
    pub async fn update_status(&self, job_id: &str, status: &str) -> Result<(), String> {
        debug!("Updating job {} status to: {}", job_id, status);

        let completed_at = if status == "completed" || status == "failed" || status == "cancelled" {
            Some(Utc::now())
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = ?, completed_at = ?
            WHERE job_id = ?
            "#
        )
        .bind(status)
        .bind(completed_at)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Complete a job with result data
    pub async fn complete_job(&self, job_id: &str, result: &Value) -> Result<(), String> {
        debug!("Completing job: {}", job_id);

        let result_json = serde_json::to_string(result)
            .map_err(|e| format!("Failed to serialize result: {}", e))?;

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'completed', result_data = ?, completed_at = CURRENT_TIMESTAMP
            WHERE job_id = ?
            "#
        )
        .bind(&result_json)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Mark a job as failed
    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), String> {
        debug!("Failing job {}: {}", job_id, error);

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'failed', error_message = ?, completed_at = CURRENT_TIMESTAMP
            WHERE job_id = ?
            "#
        )
        .bind(error)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Save a checkpoint for resume capability
    pub async fn save_checkpoint(&self, job_id: &str, checkpoint: &Value) -> Result<(), String> {
        debug!("Saving checkpoint for job: {}", job_id);

        let checkpoint_json = serde_json::to_string(checkpoint)
            .map_err(|e| format!("Failed to serialize checkpoint: {}", e))?;

        sqlx::query(
            r#"
            UPDATE background_jobs
            SET resume_checkpoint = ?
            WHERE job_id = ?
            "#
        )
        .bind(&checkpoint_json)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Get a job by ID
    pub async fn get_job(&self, job_id: &str) -> Result<Option<PersistedJob>, String> {
        let row = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, String, Option<String>, bool, Option<String>, i32, i32)>(
            r#"
            SELECT job_id, instruction, status, result_data, error_message,
                   started_at, updated_at, completed_at, resumable, resume_checkpoint,
                   retry_count, max_retries
            FROM background_jobs
            WHERE job_id = ?
            "#
        )
        .bind(job_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        match row {
            Some((job_id, instruction, status, result_data, error_message, started_at, updated_at, completed_at, resumable, resume_checkpoint, retry_count, max_retries)) => {
                Ok(Some(PersistedJob {
                    job_id,
                    instruction,
                    status,
                    result_data,
                    error_message,
                    started_at: DateTime::parse_from_rfc3339(&started_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                    completed_at: completed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
                    resumable,
                    resume_checkpoint,
                    retry_count,
                    max_retries,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get all running jobs (for resume on startup)
    pub async fn get_running_jobs(&self) -> Result<Vec<PersistedJob>, String> {
        let rows = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, String, Option<String>, bool, Option<String>, i32, i32)>(
            r#"
            SELECT job_id, instruction, status, result_data, error_message,
                   started_at, updated_at, completed_at, resumable, resume_checkpoint,
                   retry_count, max_retries
            FROM background_jobs
            WHERE status = 'running'
            ORDER BY started_at ASC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let jobs: Vec<PersistedJob> = rows.into_iter().map(|(job_id, instruction, status, result_data, error_message, started_at, updated_at, completed_at, resumable, resume_checkpoint, retry_count, max_retries)| {
            PersistedJob {
                job_id,
                instruction,
                status,
                result_data,
                error_message,
                started_at: DateTime::parse_from_rfc3339(&started_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                completed_at: completed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
                resumable,
                resume_checkpoint,
                retry_count,
                max_retries,
            }
        }).collect();

        Ok(jobs)
    }

    /// Get resumable jobs that were interrupted
    pub async fn get_resumable_jobs(&self) -> Result<Vec<PersistedJob>, String> {
        let rows = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, String, Option<String>, bool, Option<String>, i32, i32)>(
            r#"
            SELECT job_id, instruction, status, result_data, error_message,
                   started_at, updated_at, completed_at, resumable, resume_checkpoint,
                   retry_count, max_retries
            FROM background_jobs
            WHERE status = 'running' AND resumable = TRUE AND retry_count < max_retries
            ORDER BY started_at ASC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let jobs: Vec<PersistedJob> = rows.into_iter().map(|(job_id, instruction, status, result_data, error_message, started_at, updated_at, completed_at, resumable, resume_checkpoint, retry_count, max_retries)| {
            PersistedJob {
                job_id,
                instruction,
                status,
                result_data,
                error_message,
                started_at: DateTime::parse_from_rfc3339(&started_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                completed_at: completed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
                resumable,
                resume_checkpoint,
                retry_count,
                max_retries,
            }
        }).collect();

        Ok(jobs)
    }

    /// Increment retry count for a job
    pub async fn increment_retry(&self, job_id: &str) -> Result<i32, String> {
        sqlx::query(
            r#"
            UPDATE background_jobs
            SET retry_count = retry_count + 1
            WHERE job_id = ?
            "#
        )
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        // Get the new retry count
        let row = sqlx::query_as::<_, (i32,)>(
            "SELECT retry_count FROM background_jobs WHERE job_id = ?"
        )
        .bind(job_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        Ok(row.0)
    }

    /// Delete a job
    pub async fn delete_job(&self, job_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM background_jobs WHERE job_id = ?")
            .bind(job_id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(())
    }

    /// Clean up old completed/failed jobs (older than specified days)
    pub async fn cleanup_old_jobs(&self, days_old: i64) -> Result<u64, String> {
        let result = sqlx::query(
            r#"
            DELETE FROM background_jobs
            WHERE status IN ('completed', 'failed', 'cancelled')
            AND completed_at < datetime('now', ? || ' days')
            "#
        )
        .bind(-days_old)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let deleted = result.rows_affected();
        if deleted > 0 {
            info!("Cleaned up {} old background jobs", deleted);
        }

        Ok(deleted)
    }

    /// Mark interrupted jobs as failed on startup (non-resumable ones)
    pub async fn mark_interrupted_jobs_failed(&self) -> Result<u64, String> {
        let result = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'failed', error_message = 'Job interrupted by server restart', completed_at = CURRENT_TIMESTAMP
            WHERE status = 'running' AND resumable = FALSE
            "#
        )
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let updated = result.rows_affected();
        if updated > 0 {
            warn!("Marked {} interrupted jobs as failed", updated);
        }

        Ok(updated)
    }

    /// Get all jobs with optional status filter, ordered by started_at descending
    pub async fn get_all_jobs(&self, status_filter: Option<&str>, limit: Option<i64>) -> Result<Vec<PersistedJob>, String> {
        let limit_val = limit.unwrap_or(100);

        let rows = if let Some(status) = status_filter {
            sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, String, Option<String>, bool, Option<String>, i32, i32)>(
                r#"
                SELECT job_id, instruction, status, result_data, error_message,
                       started_at, updated_at, completed_at, resumable, resume_checkpoint,
                       retry_count, max_retries
                FROM background_jobs
                WHERE status = ?
                ORDER BY started_at DESC
                LIMIT ?
                "#
            )
            .bind(status)
            .bind(limit_val)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, String, String, Option<String>, bool, Option<String>, i32, i32)>(
                r#"
                SELECT job_id, instruction, status, result_data, error_message,
                       started_at, updated_at, completed_at, resumable, resume_checkpoint,
                       retry_count, max_retries
                FROM background_jobs
                ORDER BY started_at DESC
                LIMIT ?
                "#
            )
            .bind(limit_val)
            .fetch_all(&self.pool)
            .await
        };

        let rows = rows.map_err(|e| format!("Database error: {}", e))?;

        let jobs: Vec<PersistedJob> = rows.into_iter().map(|(job_id, instruction, status, result_data, error_message, started_at, updated_at, completed_at, resumable, resume_checkpoint, retry_count, max_retries)| {
            PersistedJob {
                job_id,
                instruction,
                status,
                result_data,
                error_message,
                started_at: DateTime::parse_from_rfc3339(&started_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                updated_at: DateTime::parse_from_rfc3339(&updated_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now()),
                completed_at: completed_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc))),
                resumable,
                resume_checkpoint,
                retry_count,
                max_retries,
            }
        }).collect();

        Ok(jobs)
    }

    /// Cancel a job by updating its status
    pub async fn cancel_job(&self, job_id: &str) -> Result<bool, String> {
        debug!("Cancelling job: {}", job_id);

        let result = sqlx::query(
            r#"
            UPDATE background_jobs
            SET status = 'cancelled', completed_at = CURRENT_TIMESTAMP, error_message = 'Cancelled by user'
            WHERE job_id = ? AND status = 'running'
            "#
        )
        .bind(job_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Database error: {}", e))?;

        let updated = result.rows_affected() > 0;
        if updated {
            info!("Cancelled job: {}", job_id);
        } else {
            debug!("Job {} was not running or not found", job_id);
        }

        Ok(updated)
    }
}
