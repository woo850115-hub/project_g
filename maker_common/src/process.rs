//! Game server process management — shared between MUD and 2D game makers.
//!
//! Manages starting/stopping a game server as a child process.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

/// Shared state for a managed game server process.
#[derive(Clone)]
pub struct ProcessManager {
    inner: Arc<Mutex<ProcessInner>>,
}

struct ProcessInner {
    child: Option<Child>,
    package_name: String,
    config_path: PathBuf,
    extra_args: Vec<String>,
}

impl ProcessManager {
    pub fn new(package_name: &str, config_path: PathBuf, extra_args: Vec<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ProcessInner {
                child: None,
                package_name: package_name.to_string(),
                config_path,
                extra_args,
            })),
        }
    }

    pub async fn status(&self) -> Value {
        let mut inner = self.inner.lock().await;
        if let Some(ref mut child) = inner.child {
            match child.try_wait() {
                Ok(Some(_status)) => {
                    inner.child = None;
                    serde_json::json!({"running": false})
                }
                Ok(None) => {
                    let pid = child.id().unwrap_or(0);
                    serde_json::json!({"running": true, "pid": pid})
                }
                Err(_) => {
                    inner.child = None;
                    serde_json::json!({"running": false})
                }
            }
        } else {
            serde_json::json!({"running": false})
        }
    }

    pub async fn start(&self) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;

        // Check if already running
        if let Some(ref mut child) = inner.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    inner.child = None;
                }
                Ok(None) => {
                    let pid = child.id().unwrap_or(0);
                    return Err(format!("Server already running (PID {pid})"));
                }
                Err(_) => {
                    inner.child = None;
                }
            }
        }

        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

        let mut cmd = Command::new(&cargo);
        cmd.arg("run")
            .arg("-p")
            .arg(&inner.package_name)
            .arg("--")
            .arg("--config")
            .arg(&inner.config_path);

        for arg in &inner.extra_args {
            cmd.arg(arg);
        }

        cmd.kill_on_drop(true);

        match cmd.spawn() {
            Ok(child) => {
                let pid = child.id().unwrap_or(0);
                inner.child = Some(child);
                Ok(serde_json::json!({"ok": true, "pid": pid}))
            }
            Err(e) => Err(format!("Failed to start server: {e}")),
        }
    }

    pub async fn stop(&self) -> Result<Value, String> {
        let mut inner = self.inner.lock().await;

        if let Some(ref mut child) = inner.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    inner.child = None;
                    return Err("Server already stopped".to_string());
                }
                Ok(None) => {}
                Err(_) => {
                    inner.child = None;
                    return Err("Server in unknown state".to_string());
                }
            }

            if let Err(e) = child.kill().await {
                return Err(format!("Failed to stop: {e}"));
            }

            inner.child = None;
            Ok(serde_json::json!({"ok": true}))
        } else {
            Err("No server running".to_string())
        }
    }
}

/// Trait that the app state must implement so server handlers can access ProcessManager.
pub trait HasProcessManager: Clone + Send + Sync + 'static {
    fn process_manager(&self) -> &ProcessManager;
}

/// Build server management API router.
pub fn router<S: HasProcessManager>() -> Router<S> {
    Router::new()
        .route("/status", get(server_status::<S>))
        .route("/start", post(server_start::<S>))
        .route("/stop", post(server_stop::<S>))
        .route("/restart", post(server_restart::<S>))
}

async fn server_status<S: HasProcessManager>(State(state): State<S>) -> impl IntoResponse {
    let status = state.process_manager().status().await;
    (StatusCode::OK, Json(status))
}

async fn server_start<S: HasProcessManager>(State(state): State<S>) -> impl IntoResponse {
    match state.process_manager().start().await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

async fn server_stop<S: HasProcessManager>(State(state): State<S>) -> impl IntoResponse {
    match state.process_manager().stop().await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

async fn server_restart<S: HasProcessManager>(State(state): State<S>) -> impl IntoResponse {
    let pm = state.process_manager();
    let _ = pm.stop().await;
    // Brief delay to ensure port is released
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    match pm.start().await {
        Ok(v) => (StatusCode::OK, Json(v)),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
    }
}
