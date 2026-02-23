//! File-based script CRUD — shared between MUD and 2D game makers.
//!
//! Manages Lua script files under `scripts_dir/`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;

/// Trait that the app state must implement so script handlers can locate files.
pub trait ScriptsDir: Clone + Send + Sync + 'static {
    fn scripts_dir(&self) -> PathBuf;

    /// Files to exclude from listing (e.g. auto-generated world setup scripts).
    fn excluded_scripts(&self) -> Vec<String> {
        vec![]
    }
}

/// Build scripts API router for any state that implements ScriptsDir.
pub fn router<S: ScriptsDir>() -> Router<S> {
    Router::new()
        .route("/", get(list_scripts::<S>).post(create_script::<S>))
        .route(
            "/{filename}",
            get(get_script::<S>)
                .put(update_script::<S>)
                .delete(delete_script::<S>),
        )
}

#[derive(Deserialize)]
struct ScriptBody {
    content: Option<String>,
    filename: Option<String>,
}

/// GET /api/scripts — list all script files
async fn list_scripts<S: ScriptsDir>(State(state): State<S>) -> impl IntoResponse {
    let scripts_dir = state.scripts_dir();
    let excluded = state.excluded_scripts();

    if !scripts_dir.exists() {
        return (StatusCode::OK, Json(serde_json::json!([])));
    }

    let mut files: Vec<Value> = Vec::new();

    let entries = match std::fs::read_dir(&scripts_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read scripts dir: {e}")})),
            );
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lua") {
            continue;
        }
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if excluded.contains(&filename) {
            continue;
        }

        let metadata = entry.metadata().ok();
        let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);

        files.push(serde_json::json!({
            "filename": filename,
            "size": size,
        }));
    }

    files.sort_by(|a, b| {
        let fa = a["filename"].as_str().unwrap_or("");
        let fb = b["filename"].as_str().unwrap_or("");
        fa.cmp(fb)
    });

    (StatusCode::OK, Json(serde_json::json!(files)))
}

/// GET /api/scripts/:filename — get script content
async fn get_script<S: ScriptsDir>(
    State(state): State<S>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    if !is_valid_lua_filename(&filename) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid filename"})),
        );
    }

    let path = state.scripts_dir().join(&filename);
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Script not found"})),
        );
    }

    match std::fs::read_to_string(&path) {
        Ok(content) => (
            StatusCode::OK,
            Json(serde_json::json!({"filename": filename, "content": content})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to read: {e}")})),
        ),
    }
}

/// POST /api/scripts — create a new script file
async fn create_script<S: ScriptsDir>(
    State(state): State<S>,
    Json(body): Json<ScriptBody>,
) -> impl IntoResponse {
    let filename = match &body.filename {
        Some(f) => f.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Missing 'filename' field"})),
            );
        }
    };

    if !is_valid_lua_filename(&filename) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid filename. Must end with .lua"})),
        );
    }

    let scripts_dir = state.scripts_dir();
    if !scripts_dir.exists() {
        let _ = std::fs::create_dir_all(&scripts_dir);
    }

    let path = scripts_dir.join(&filename);
    if path.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Script already exists"})),
        );
    }

    let content = body.content.as_deref().unwrap_or("");
    match std::fs::write(&path, content) {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write: {e}")})),
        ),
    }
}

/// PUT /api/scripts/:filename — update script content
async fn update_script<S: ScriptsDir>(
    State(state): State<S>,
    Path(filename): Path<String>,
    Json(body): Json<ScriptBody>,
) -> impl IntoResponse {
    if !is_valid_lua_filename(&filename) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid filename"})),
        );
    }

    let content = match &body.content {
        Some(c) => c,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Missing 'content' field"})),
            );
        }
    };

    let path = state.scripts_dir().join(&filename);
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Script not found"})),
        );
    }

    match std::fs::write(&path, content) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write: {e}")})),
        ),
    }
}

/// DELETE /api/scripts/:filename — delete a script file
async fn delete_script<S: ScriptsDir>(
    State(state): State<S>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    if !is_valid_lua_filename(&filename) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid filename"})),
        );
    }

    let path = state.scripts_dir().join(&filename);
    if !path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Script not found"})),
        );
    }

    match std::fs::remove_file(&path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to delete: {e}")})),
        ),
    }
}

fn is_valid_lua_filename(name: &str) -> bool {
    name.ends_with(".lua")
        && name.len() > 4
        && name.len() <= 128
        && !name.contains('/')
        && !name.contains('\\')
        && !name.contains("..")
}
