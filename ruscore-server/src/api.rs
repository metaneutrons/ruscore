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

#[derive(Deserialize)]
pub struct CreateJobRequest {
    url: String,
}

#[derive(Serialize)]
pub struct CreateJobResponse {
    id: Uuid,
    status: JobStatus,
}

#[derive(Deserialize)]
pub struct ListParams {
    page: Option<i64>,
    per_page: Option<i64>,
    status: Option<String>,
    sort: Option<String>,
    order: Option<String>,
    q: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

/// POST /api/v1/jobs — submit a URL for conversion.
pub async fn create_job(
    State(state): State<AppState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<(StatusCode, Json<CreateJobResponse>), AppError> {
    let url_hash = hex_sha256(&req.url);
    let id = Uuid::new_v4();

    let existing = state.db.insert(id, &req.url, &url_hash)?;

    if let Some(job) = existing {
        return Ok((
            StatusCode::CONFLICT,
            Json(CreateJobResponse {
                id: job.id,
                status: job.status,
            }),
        ));
    }

    state.job_notify.notify_one();

    Ok((
        StatusCode::ACCEPTED,
        Json(CreateJobResponse {
            id,
            status: JobStatus::Queued,
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
    let list = state.db.list(
        page,
        per_page,
        params.status.as_deref(),
        params.sort.as_deref(),
        params.order.as_deref(),
        params.q.as_deref(),
    )?;
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
    match state.db.get_pdf(id)? {
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
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
