//! API route handlers.

use crate::db::JobStatus;
use crate::state::AppState;
use axum::Json;
use axum::extract::{
    Path, Query, State,
    rejection::{JsonRejection, PathRejection, QueryRejection},
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// --- RFC 7807 Problem Details ---

const PROBLEM_JSON: &str = "application/problem+json";

#[derive(Serialize)]
struct ProblemDetail {
    r#type: &'static str,
    title: &'static str,
    status: u16,
    detail: String,
}

impl ProblemDetail {
    fn response(status: StatusCode, title: &'static str, detail: impl Into<String>) -> Response {
        let body = Self {
            r#type: "about:blank",
            title,
            status: status.as_u16(),
            detail: detail.into(),
        };
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static(PROBLEM_JSON));
        (status, headers, Json(body)).into_response()
    }

    fn not_found(detail: impl Into<String>) -> Response {
        Self::response(StatusCode::NOT_FOUND, "Not Found", detail)
    }

    fn bad_request(detail: impl Into<String>) -> Response {
        Self::response(StatusCode::BAD_REQUEST, "Bad Request", detail)
    }

    fn conflict(detail: impl Into<String>) -> Response {
        Self::response(StatusCode::CONFLICT, "Conflict", detail)
    }

    fn unprocessable(detail: impl Into<String>) -> Response {
        Self::response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Entity",
            detail,
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

#[derive(Serialize)]
pub struct SuggestResult {
    id: String,
    title: String,
    composer: String,
}

#[derive(Deserialize)]
pub struct BatchDeleteRequest {
    ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct BatchDeleteResponse {
    deleted: usize,
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

/// Validate that the URL is a MuseScore score URL.
fn validate_musescore_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid URL: {e}"))?;
    match parsed.host_str() {
        Some(h) if h == "musescore.com" || h.ends_with(".musescore.com") => {}
        _ => return Err("URL must be a musescore.com score page".into()),
    }
    if !parsed.path().contains("/scores/") {
        return Err("URL must point to a MuseScore score (path must contain /scores/)".into());
    }
    Ok(())
}

// --- Handlers ---

/// POST /api/v1/jobs — submit a URL for conversion.
pub async fn create_job(
    State(state): State<AppState>,
    body: Result<Json<CreateJobRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(req) = body.map_err(AppError::from)?;
    if let Err(msg) = validate_musescore_url(&req.url) {
        return Ok(ProblemDetail::unprocessable(msg));
    }

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
    params: Result<Query<ListParams>, QueryRejection>,
) -> Result<Response, AppError> {
    let Query(params) = params.map_err(AppError::from)?;
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

    let mut links = Vec::new();
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
    links.push(format!(
        "<{}>; rel=\"first\"",
        build_list_url(&params, 1, per_page)
    ));
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
    params: Result<Query<SuggestParams>, QueryRejection>,
) -> Result<Json<Vec<SuggestResult>>, AppError> {
    let Query(params) = params.map_err(AppError::from)?;
    let rows = state
        .db
        .suggest(&params.q, params.limit.unwrap_or(5).clamp(1, 20))?;
    let results = rows
        .into_iter()
        .map(|v| SuggestResult {
            id: v
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            title: v
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            composer: v
                .get("composer")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        })
        .collect();
    Ok(Json(results))
}

/// GET /api/v1/jobs/:id — job status + metadata.
pub async fn get_job(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(AppError::from)?;
    match state.db.get(id)? {
        Some(job) => Ok(Json(job).into_response()),
        None => Ok(ProblemDetail::not_found(format!("Job {id} not found"))),
    }
}

/// GET /api/v1/jobs/:id/pdf — download the generated PDF.
pub async fn get_pdf(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(AppError::from)?;
    let job = match state.db.get(id)? {
        Some(j) => j,
        None => return Ok(ProblemDetail::not_found(format!("Job {id} not found"))),
    };

    if job.status != JobStatus::Completed {
        return Ok(ProblemDetail::conflict(format!(
            "PDF not ready — job status is '{:?}'",
            job.status
        )));
    }

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
        None => Ok(ProblemDetail::not_found("PDF data not found")),
    }
}

/// DELETE /api/v1/jobs/:id — delete a single job.
pub async fn delete_job(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let Path(id) = id.map_err(AppError::from)?;
    if !is_confirmed(&headers) {
        return Ok(ProblemDetail::bad_request(
            "Set header 'X-Confirm: yes' to confirm deletion",
        ));
    }
    match state.db.delete(id)? {
        true => Ok(StatusCode::NO_CONTENT.into_response()),
        false => Ok(ProblemDetail::not_found(format!("Job {id} not found"))),
    }
}

/// POST /api/v1/jobs/batch/delete — bulk delete jobs.
pub async fn batch_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Result<Json<BatchDeleteRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let Json(body) = body.map_err(AppError::from)?;
    if !is_confirmed(&headers) {
        return Ok(ProblemDetail::bad_request(
            "Set header 'X-Confirm: yes' to confirm deletion",
        ));
    }
    let deleted = state.db.delete_many(&body.ids)?;
    Ok(Json(BatchDeleteResponse { deleted }).into_response())
}

/// GET /health
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

// --- Error handling (RFC 7807) ---

pub struct AppError {
    status: StatusCode,
    title: &'static str,
    detail: String,
}

impl AppError {
    fn internal(err: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            title: "Internal Server Error",
            detail: err.to_string(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!("Request error: {}", self.detail);
        }
        ProblemDetail::response(self.status, self.title, self.detail)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::internal(err)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::internal(err)
    }
}

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            title: "Bad Request",
            detail: rejection.body_text(),
        }
    }
}

impl From<PathRejection> for AppError {
    fn from(rejection: PathRejection) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            title: "Bad Request",
            detail: rejection.body_text(),
        }
    }
}

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            title: "Bad Request",
            detail: rejection.body_text(),
        }
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
        parts.push(format!("status={}", urlencoded(s)));
    }
    if let Some(ref s) = params.sort {
        parts.push(format!("sort={}", urlencoded(s)));
    }
    if let Some(ref o) = params.order {
        parts.push(format!("order={}", urlencoded(o)));
    }
    if let Some(ref q) = params.q {
        parts.push(format!("q={}", urlencoded(q)));
    }
    format!("/api/v1/jobs?{}", parts.join("&"))
}

fn urlencoded(s: &str) -> String {
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace(' ', "%20")
        .replace('+', "%2B")
}
