//! ruscore web service — MuseScore score scraping API with job queue.

#![forbid(unsafe_code)]
#![warn(clippy::redundant_closure)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::uninlined_format_args)]

mod api;
mod cache;
mod db;
mod state;
mod worker;

use anyhow::{Context, Result};
use axum::Router;
use axum::routing::{get, post};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::cache::PdfCache;
use crate::db::JobDb;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".parse().expect("valid filter")),
        )
        .init();

    let port: u16 = std::env::var("RUSCORE_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .context("invalid RUSCORE_PORT")?;
    let redis_url =
        std::env::var("RUSCORE_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let data_dir =
        PathBuf::from(std::env::var("RUSCORE_DATA_DIR").unwrap_or_else(|_| "./data".into()));

    std::fs::create_dir_all(&data_dir).context("failed to create data dir")?;

    let db_path = data_dir.join("ruscore.db");
    let db = Arc::new(JobDb::open(db_path.to_str().unwrap())?);
    let cache = Arc::new(PdfCache::connect(&redis_url).await?);
    let job_notify = Arc::new(Notify::new());

    let state = AppState {
        db: Arc::clone(&db),
        cache: Arc::clone(&cache),
        job_notify: Arc::clone(&job_notify),
    };

    // Start background worker
    let worker_state = state.clone();
    let worker_dir = data_dir.clone();
    let worker_notify = Arc::clone(&job_notify);
    tokio::spawn(async move {
        worker::run(worker_state, worker_dir, worker_notify).await;
    });

    // Build router
    let app = Router::new()
        .route("/api/v1/jobs", post(api::create_job).get(api::list_jobs))
        .route("/api/v1/jobs/{id}", get(api::get_job))
        .route("/api/v1/jobs/{id}/pdf", get(api::get_pdf))
        .route("/health", get(api::health))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
    info!("Shutting down...");
}
