//! File-based content CRUD — shared between MUD and 2D game makers.
//!
//! Each "collection" is a JSON array file under `content_dir/`.
//! e.g. `content/monsters.json` = [{"id":"goblin",...}, ...]

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::Value;
use std::path::{Path as StdPath, PathBuf};

/// Trait that the app state must implement so content handlers can locate files.
pub trait ContentDir: Clone + Send + Sync + 'static {
    fn content_dir(&self) -> PathBuf;
}

/// Build content API router for any state that implements ContentDir.
pub fn router<S: ContentDir>() -> Router<S> {
    Router::new()
        .route("/", get(list_collections::<S>).post(create_collection::<S>))
        .route(
            "/{collection}",
            get(list_items::<S>).delete(delete_collection::<S>),
        )
        .route(
            "/{collection}/{id}",
            get(get_item::<S>)
                .put(update_item::<S>)
                .delete(delete_item::<S>),
        )
}

/// GET /api/content — list all collection names
async fn list_collections<S: ContentDir>(State(state): State<S>) -> impl IntoResponse {
    let content_dir = state.content_dir();

    if !content_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&content_dir) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to create content dir: {e}")})),
            );
        }
    }

    let mut collections: Vec<String> = Vec::new();

    let entries = match std::fs::read_dir(&content_dir) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read content dir: {e}")})),
            );
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                collections.push(stem.to_string());
            }
        }
    }

    collections.sort();
    (StatusCode::OK, Json(serde_json::json!(collections)))
}

/// POST /api/content — create a new collection
async fn create_collection<S: ContentDir>(
    State(state): State<S>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let id = match body.get("id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Missing 'id' field"})),
            );
        }
    };

    if !is_valid_name(&id) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid collection name. Use alphanumeric and underscore only."})),
        );
    }

    let content_dir = state.content_dir();
    if !content_dir.exists() {
        let _ = std::fs::create_dir_all(&content_dir);
    }

    let file_path = content_dir.join(format!("{id}.json"));
    if file_path.exists() {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error": "Collection already exists"})),
        );
    }

    match std::fs::write(&file_path, "[]") {
        Ok(()) => (StatusCode::CREATED, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to create file: {e}")})),
        ),
    }
}

/// GET /api/content/:collection — list all items in a collection
async fn list_items<S: ContentDir>(
    State(state): State<S>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    match read_collection(&state.content_dir(), &collection) {
        Ok(items) => (StatusCode::OK, Json(serde_json::json!(items))),
        Err(e) => e,
    }
}

/// DELETE /api/content/:collection — delete entire collection
async fn delete_collection<S: ContentDir>(
    State(state): State<S>,
    Path(collection): Path<String>,
) -> impl IntoResponse {
    let file_path = collection_path(&state.content_dir(), &collection);
    if !file_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Collection not found"})),
        );
    }

    match std::fs::remove_file(&file_path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to delete: {e}")})),
        ),
    }
}

/// GET /api/content/:collection/:id — get a single item
async fn get_item<S: ContentDir>(
    State(state): State<S>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let items = match read_collection(&state.content_dir(), &collection) {
        Ok(items) => items,
        Err(e) => return e,
    };

    match find_item(&items, &id) {
        Some(item) => (StatusCode::OK, Json(item.clone())),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Item not found"})),
        ),
    }
}

/// PUT /api/content/:collection/:id — create or update an item
async fn update_item<S: ContentDir>(
    State(state): State<S>,
    Path((collection, id)): Path<(String, String)>,
    Json(mut body): Json<Value>,
) -> impl IntoResponse {
    // Ensure the body has the correct id
    if let Value::Object(ref mut map) = body {
        map.insert("id".to_string(), Value::String(id.clone()));
    }

    let content_dir = state.content_dir();
    let mut items = match read_collection(&content_dir, &collection) {
        Ok(items) => items,
        Err(e) => return e,
    };

    // Find and replace, or append
    let mut found = false;
    for item in items.iter_mut() {
        if item.get("id").and_then(|v| v.as_str()) == Some(&id) {
            *item = body.clone();
            found = true;
            break;
        }
    }

    if !found {
        items.push(body);
    }

    match write_collection(&content_dir, &collection, &items) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

/// DELETE /api/content/:collection/:id — delete a single item
async fn delete_item<S: ContentDir>(
    State(state): State<S>,
    Path((collection, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let content_dir = state.content_dir();
    let mut items = match read_collection(&content_dir, &collection) {
        Ok(items) => items,
        Err(e) => return e,
    };

    let len_before = items.len();
    items.retain(|item| item.get("id").and_then(|v| v.as_str()) != Some(&id));

    if items.len() == len_before {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Item not found"})),
        );
    }

    match write_collection(&content_dir, &collection, &items) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => e,
    }
}

// --- Helpers ---

fn collection_path(content_dir: &StdPath, name: &str) -> PathBuf {
    content_dir.join(format!("{name}.json"))
}

pub fn read_collection(
    content_dir: &StdPath,
    name: &str,
) -> Result<Vec<Value>, (StatusCode, Json<Value>)> {
    let path = collection_path(content_dir, name);
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Collection not found"})),
        ));
    }

    let text = std::fs::read_to_string(&path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to read file: {e}")})),
        )
    })?;

    let items: Vec<Value> = serde_json::from_str(&text).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Invalid JSON: {e}")})),
        )
    })?;

    Ok(items)
}

pub fn write_collection(
    content_dir: &StdPath,
    name: &str,
    items: &[Value],
) -> Result<(), (StatusCode, Json<Value>)> {
    let path = collection_path(content_dir, name);
    let json = serde_json::to_string_pretty(items).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("JSON serialize error: {e}")})),
        )
    })?;

    std::fs::write(&path, json).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write file: {e}")})),
        )
    })?;

    Ok(())
}

fn find_item<'a>(items: &'a [Value], id: &str) -> Option<&'a Value> {
    items
        .iter()
        .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(id))
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
