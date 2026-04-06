//! API route handlers.

use crate::db::{JobList, JobStatus};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// --- Request / Response types ---

#[derive(Deserialize)]
pub struct CreateJobRequest {
    url: String,
}

#[derive(Serialize)]
pub struct CreateJobResponse {
    id: Uuid,
    status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached: Option<bool>,
}

#[derive(Deserialize)]
pub struct ListParams {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

// --- Handlers ---

/// POST /api/v1/jobs — submit a URL for conversion.
pub async fn create_job(
    State(state): State<AppState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<CreateJobResponse>), AppError> {
    let url_hash = hex_sha256(&req.url);
    let id = Uuid::new_v4();

    // Insert or get existing (dedup by URL hash)
    let existing = state.db.insert(id, &req.url, &url_hash)?;

    if let Some(job) = existing {
        // URL already submitted — return existing job
        return Ok((
            StatusCode::CONFLICT,
            Json(CreateJobResponse {
                id: job.id,
                status: job.status,
                cached: None,
            }),
        ));
    }

    // Check if PDF is already cached in Redis
    let cached = state.cache.exists(&url_hash).await?;
    if cached {
        // Mark as completed immediately
        state.db.complete(id, &serde_json::Value::Null, 0)?;
    }

    // Wake the worker
    state.job_notify.notify_one();

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateJobResponse {
            id,
            status: if cached {
                JobStatus::Completed
            } else {
                JobStatus::Queued
            },
            cached: if cached { Some(true) } else { None },
        }),
    ))
}

/// GET /api/v1/jobs — paginated job list.
pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<JobList>, AppError> {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(20).clamp(1, 100);
    let status = params.status.as_deref();
    let list = state.db.list(page, per_page, status)?;
    Ok(Json(list))
}

/// GET /api/v1/jobs/:id — job status + metadata.
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    match state.db.get(id)? {
        Some(job) => Ok(Json(job).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// GET /api/v1/jobs/:id/pdf — download the generated PDF.
pub async fn get_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let job = match state.db.get(id)? {
        Some(j) if j.status == JobStatus::Completed => j,
        Some(_) => return Ok(StatusCode::NOT_FOUND.into_response()),
        None => return Ok(StatusCode::NOT_FOUND.into_response()),
    };

    match state.cache.get(&job.url_hash).await? {
        Some(bytes) => Ok((
            StatusCode::OK,
            [
                ("content-type", "application/pdf"),
                ("content-disposition", "inline; filename=\"score.pdf\""),
            ],
            bytes,
        )
            .into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

/// GET /health
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

// --- Error handling ---

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Request error: {:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": self.0.to_string()})),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    result.iter().map(|b| format!("{b:02x}")).collect()
}
