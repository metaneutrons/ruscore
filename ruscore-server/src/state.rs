//! Shared application state.

use crate::db::JobDb;
use std::sync::Arc;
use tokio::sync::Notify;

/// Shared state accessible from all request handlers and the background worker.
#[derive(Clone)]
pub struct AppState {
    /// SQLite job database.
    pub db: Arc<JobDb>,
    /// Notify the worker when a new job is queued.
    pub job_notify: Arc<Notify>,
}
