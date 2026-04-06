//! SQLite job persistence.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

/// Job status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Waiting in queue.
    Queued,
    /// Currently being processed.
    Processing,
    /// PDF generated successfully.
    Completed,
    /// Processing failed.
    Failed,
}

impl JobStatus {
    fn from_str(s: &str) -> Self {
        match s {
            "queued" => Self::Queued,
            "processing" => Self::Processing,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Failed,
        }
    }
}

/// A conversion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique job ID.
    pub id: Uuid,
    /// MuseScore URL.
    pub url: String,
    /// SHA-256 hash of the URL (for dedup + cache key).
    pub url_hash: String,
    /// Current status.
    pub status: JobStatus,
    /// Extracted metadata (JSON), populated on completion.
    pub metadata: Option<serde_json::Value>,
    /// Number of pages, populated on completion.
    pub pages: Option<i64>,
    /// PDF binary data, populated on completion.
    #[serde(skip)]
    #[allow(dead_code)]
    pub pdf_data: Option<Vec<u8>>,
    /// Error message if failed.
    pub error: Option<String>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last update timestamp.
    pub updated_at: String,
}

/// Paginated job list response.
#[derive(Debug, Serialize)]
pub struct JobList {
    /// Jobs on this page.
    pub jobs: Vec<Job>,
    /// Total matching jobs.
    pub total: i64,
    /// Current page (1-based).
    pub page: i64,
    /// Items per page.
    pub per_page: i64,
}

/// SQLite-backed job database.
pub struct JobDb {
    conn: Mutex<Connection>,
}

impl JobDb {
    /// Open (or create) the database at the given path.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path).context("failed to open SQLite database")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                url_hash TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'queued',
                metadata TEXT,
                pages INTEGER,
                pdf_data BLOB,
                error TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_jobs_url_hash ON jobs(url_hash);
            CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);
            CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs(created_at);",
        )
        .context("failed to initialize schema")?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insert a new queued job. Returns None if url_hash already exists.
    pub fn insert(&self, id: Uuid, url: &str, url_hash: &str) -> Result<Option<Job>> {
        let conn = self.conn.lock().unwrap();
        // Check for existing job with same URL hash
        let existing: Option<Job> = conn
            .query_row(
                "SELECT id, url, url_hash, status, metadata, pages, error, created_at, updated_at FROM jobs WHERE url_hash = ?1",
                params![url_hash],
                |row| Ok(row_to_job(row)),
            )
            .ok();

        if let Some(job) = existing {
            return Ok(Some(job));
        }

        conn.execute(
            "INSERT INTO jobs (id, url, url_hash) VALUES (?1, ?2, ?3)",
            params![id.to_string(), url, url_hash],
        )?;
        self.get_by_id_inner(&conn, id)
    }

    /// Get a job by ID.
    pub fn get(&self, id: Uuid) -> Result<Option<Job>> {
        let conn = self.conn.lock().unwrap();
        self.get_by_id_inner(&conn, id)
    }

    fn get_by_id_inner(&self, conn: &Connection, id: Uuid) -> Result<Option<Job>> {
        Ok(conn
            .query_row(
                "SELECT id, url, url_hash, status, metadata, pages, error, created_at, updated_at FROM jobs WHERE id = ?1",
                params![id.to_string()],
                |row| Ok(row_to_job(row)),
            )
            .ok())
    }

    /// Get the next queued job and mark it as processing.
    pub fn claim_next(&self) -> Result<Option<Job>> {
        let conn = self.conn.lock().unwrap();
        let job: Option<Job> = conn
            .query_row(
                "SELECT id, url, url_hash, status, metadata, pages, error, created_at, updated_at FROM jobs WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
                [],
                |row| Ok(row_to_job(row)),
            )
            .ok();

        if let Some(ref job) = job {
            conn.execute(
                "UPDATE jobs SET status = 'processing', updated_at = datetime('now') WHERE id = ?1",
                params![job.id.to_string()],
            )?;
        }
        Ok(job)
    }

    /// Mark a job as completed with metadata and PDF.
    pub fn complete(
        &self,
        id: Uuid,
        metadata: &serde_json::Value,
        pages: i64,
        pdf_data: &[u8],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status = 'completed', metadata = ?1, pages = ?2, pdf_data = ?3, updated_at = datetime('now') WHERE id = ?4",
            params![metadata.to_string(), pages, pdf_data, id.to_string()],
        )?;
        Ok(())
    }

    /// Mark a job as failed.
    pub fn fail(&self, id: Uuid, error: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE jobs SET status = 'failed', error = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![error, id.to_string()],
        )?;
        Ok(())
    }

    /// Get PDF bytes for a completed job.
    pub fn get_pdf(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT pdf_data FROM jobs WHERE id = ?1 AND status = 'completed'",
                params![id.to_string()],
                |row| row.get(0),
            )
            .ok()
            .flatten())
    }

    /// Delete jobs older than the given number of hours.
    #[allow(dead_code)]
    pub fn cleanup(&self, max_age_hours: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let deleted = conn.execute(
            "DELETE FROM jobs WHERE created_at < datetime('now', ?1)",
            params![format!("-{max_age_hours} hours")],
        )?;
        Ok(deleted)
    }

    /// Paginated job list with optional status filter.
    pub fn list(&self, page: i64, per_page: i64, status: Option<&str>) -> Result<JobList> {
        let conn = self.conn.lock().unwrap();
        let offset = (page - 1) * per_page;

        let (where_clause, count_sql, list_sql);
        if let Some(s) = status {
            where_clause = format!("WHERE status = '{s}'");
        } else {
            where_clause = String::new();
        }
        count_sql = format!("SELECT COUNT(*) FROM jobs {where_clause}");
        list_sql = format!(
            "SELECT id, url, url_hash, status, metadata, pages, error, created_at, updated_at FROM jobs {where_clause} ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        );

        let total: i64 = conn.query_row(&count_sql, [], |row| row.get(0))?;

        let mut stmt = conn.prepare(&list_sql)?;
        let jobs = stmt
            .query_map(params![per_page, offset], |row| Ok(row_to_job(row)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(JobList {
            jobs,
            total,
            page,
            per_page,
        })
    }
}

fn row_to_job(row: &rusqlite::Row) -> Job {
    let id_str: String = row.get_unwrap(0);
    let metadata_str: Option<String> = row.get_unwrap(4);
    Job {
        id: Uuid::parse_str(&id_str).unwrap_or_default(),
        url: row.get_unwrap(1),
        url_hash: row.get_unwrap(2),
        status: JobStatus::from_str(&row.get_unwrap::<_, String>(3)),
        metadata: metadata_str.and_then(|s| serde_json::from_str(&s).ok()),
        pages: row.get_unwrap(5),
        pdf_data: None, // Never loaded in list/get queries — use get_pdf() instead
        error: row.get_unwrap(6),
        created_at: row.get_unwrap(7),
        updated_at: row.get_unwrap(8),
    }
}
