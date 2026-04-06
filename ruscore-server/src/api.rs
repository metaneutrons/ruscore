//! API route handlers.

use crate::db::JobStatus;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// --- RFC 7807 Problem Details ---

#[derive(Serialize)]
struct ProblemDetail {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}

impl ProblemDetail {
    fn not_found(detail: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::NOT_FOUND,
            Json(Self {
                r#type: "about:blank",
                title: "Not Found",
                status: 404,
                detail: detail.into(),
            }),
        )
    }

    fn bad_request(detail: impl Into<String>) -> (StatusCode, Json<Self>) {
        (
            StatusCode::BAD_REQUEST,
            Json(Self {
                r#type: "about:blank",
                title: "Bad Request",
                status: 400,
                detail: detail.into(),
            }),
        )
    }
}

// --- Request / Response types ---

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

#[derive(Deserialize)]
pub struct SuggestParams {
    q: String,
    limit: Option<i64>,
}

#[derive(Deserialize)]
pub struct BatchDeleteRequest {
    ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
}

// --- Confirmation guard via X-Confirm header ---

const CONFIRM_HEADER: &str = "x-confirm";

fn is_confirmed(headers: &HeaderMap) -> bool {
    headers.get(CONFIRM_HEADER).and_then(|v| v.to_str().ok()) == Some("yes")
}

// --- Handlers ---

/// POST /api/v1/jobs — submit a URL for conversion.
/// Returns 202 with Location header, or 409 if URL already submitted.
pub async fn create_job(
    State(state): State<AppState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Response, AppError> {
    let url_hash = hex_sha256(&req.url);
    let id = Uuid::new_v4();

    let existing = state.db.insert(id, &req.url, &url_hash)?;

    if let Some(job) = existing {
        let mut headers = HeaderMap::new();
        headers.insert(
            "location",
            HeaderValue::from_str(&format!("/api/v1/jobs/{}", job.id)).unwrap(),
        );
        return Ok((
            StatusCode::CONFLICT,
            headers,
            Json(CreateJobResponse {
                id: job.id,
                status: job.status,
            }),
        )
            .into_response());
    }

    state.job_notify.notify_one();

    let mut headers = HeaderMap::new();
    headers.insert(
        "location",
        HeaderValue::from_str(&format!("/api/v1/jobs/{id}")).unwrap(),
    );

    Ok((
        StatusCode::ACCEPTED,
        headers,
        Json(CreateJobResponse {
            id,
            status: JobStatus::Queued,
        }),
    )
        .into_response())
}

/// GET /api/v1/jobs — paginated job list with Link headers.
pub async fn list_jobs(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Response, AppError> {
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

    let total_pages = (list.total as f64 / per_page as f64).ceil() as i64;

    // Build Link headers for pagination
    let mut links = Vec::new();
    let base = build_list_url(&params, 1, per_page);
    if page > 1 {
        links.push(format!(
            "<{}>; rel=\"prev\"",
            build_list_url(&params, page - 1, per_page)
        ));
    }
    if page < total_pages {
        links.push(format!(
            "<{}>; rel=\"next\"",
            build_list_url(&params, page + 1, per_page)
        ));
    }
    links.push(format!("<{}>; rel=\"first\"", base));
    links.push(format!(
        "<{}>; rel=\"last\"",
        build_list_url(&params, total_pages.max(1), per_page)
    ));

    let mut headers = HeaderMap::new();
    if !links.is_empty() {
        headers.insert("link", HeaderValue::from_str(&links.join(", ")).unwrap());
    }

    Ok((StatusCode::OK, headers, Json(list)).into_response())
}

/// GET /api/v1/jobs/suggest — typeahead search suggestions.
pub async fn suggest(
    State(state): State<AppState>,
    Query(params): Query<SuggestParams>,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let results = state
        .db
        .suggest(&params.q, params.limit.unwrap_or(5).clamp(1, 20))?;
    Ok(Json(results))
}

/// GET /api/v1/jobs/:id — job status + metadata.
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    match state.db.get(id)? {
        Some(job) => Ok(Json(job).into_response()),
        None => Ok(ProblemDetail::not_found(format!("Job {id} not found")).into_response()),
    }
}

/// GET /api/v1/jobs/:id/pdf — download the generated PDF.
pub async fn get_pdf(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let job = match state.db.get(id)? {
        Some(j) if j.status == JobStatus::Completed => j,
        Some(_) => {
            return Ok(
                ProblemDetail::not_found("PDF not ready — job is still processing").into_response(),
            );
        }
        None => return Ok(ProblemDetail::not_found(format!("Job {id} not found")).into_response()),
    };

    match state.db.get_pdf(id)? {
        Some(bytes) => {
            let len = bytes.len();
            let title = job
                .metadata
                .as_ref()
                .and_then(|m| m.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("score");
            let filename = format!("{title}.pdf").replace(['/', '\\', '"'], "_");

            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("application/pdf"));
            headers.insert(
                "content-disposition",
                HeaderValue::from_str(&format!("inline; filename=\"{filename}\"")).unwrap(),
            );
            headers.insert(
                "content-length",
                HeaderValue::from_str(&len.to_string()).unwrap(),
            );

            Ok((StatusCode::OK, headers, bytes).into_response())
        }
        None => Ok(ProblemDetail::not_found("PDF data not found").into_response()),
    }
}

/// DELETE /api/v1/jobs/:id — delete a single job.
/// Requires X-Confirm: yes header.
pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    if !is_confirmed(&headers) {
        return Ok(
            ProblemDetail::bad_request("Set header 'X-Confirm: yes' to confirm deletion")
                .into_response(),
        );
    }
    match state.db.delete(id)? {
        true => Ok(StatusCode::NO_CONTENT.into_response()),
        false => Ok(ProblemDetail::not_found(format!("Job {id} not found")).into_response()),
    }
}

/// POST /api/v1/jobs/batch/delete — bulk delete jobs.
/// Requires X-Confirm: yes header.
pub async fn batch_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<BatchDeleteRequest>,
) -> Result<Response, AppError> {
    if !is_confirmed(&headers) {
        return Ok(
            ProblemDetail::bad_request("Set header 'X-Confirm: yes' to confirm deletion")
                .into_response(),
        );
    }
    let deleted = state.db.delete_many(&body.ids)?;
    Ok(Json(serde_json::json!({"deleted": deleted})).into_response())
}

/// GET /health
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

// --- Error handling (RFC 7807) ---

pub struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Request error: {:#}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ProblemDetail {
                r#type: "about:blank",
                title: "Internal Server Error",
                status: 500,
                detail: self.0.to_string(),
            }),
        )
            .into_response()
    }
}

impl<E: Into<anyhow::Error>> From<E> for AppError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// --- Helpers ---

fn hex_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn build_list_url(params: &ListParams, page: i64, per_page: i64) -> String {
    let mut parts = vec![format!("page={page}"), format!("per_page={per_page}")];
    if let Some(ref s) = params.status {
        parts.push(format!("status={s}"));
    }
    if let Some(ref s) = params.sort {
        parts.push(format!("sort={s}"));
    }
    if let Some(ref o) = params.order {
        parts.push(format!("order={o}"));
    }
    if let Some(ref q) = params.q {
        parts.push(format!("q={q}"));
    }
    format!("/api/v1/jobs?{}", parts.join("&"))
}
