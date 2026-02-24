use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::AppState;
use maker_common::content::ContentDir;

/// A single attribute schema entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttributeSchema {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    pub value_type: String,
    #[serde(default = "default_value")]
    pub default: Value,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

fn default_value() -> Value {
    Value::Null
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_schemas).put(save_schemas))
}

/// GET /api/attribute-schemas — list all schemas
async fn list_schemas(State(state): State<AppState>) -> impl IntoResponse {
    match load_schemas(&state) {
        Ok(schemas) => (StatusCode::OK, Json(serde_json::to_value(schemas).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

/// PUT /api/attribute-schemas — overwrite the full array
async fn save_schemas(
    State(state): State<AppState>,
    Json(schemas): Json<Vec<AttributeSchema>>,
) -> impl IntoResponse {
    let path = schema_path(&state);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json = match serde_json::to_string_pretty(&schemas) {
        Ok(j) => j,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Serialize error: {e}")})),
            )
        }
    };

    match std::fs::write(&path, json) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Write error: {e}")})),
        ),
    }
}

fn schema_path(state: &AppState) -> std::path::PathBuf {
    state.content_dir().join("attribute_schema.json")
}

fn load_schemas(state: &AppState) -> Result<Vec<AttributeSchema>, String> {
    let path = schema_path(state);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Read error: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))
}

/// Public sync loader for use in world.rs Lua generation.
pub fn load_schemas_sync(state: &AppState) -> Vec<AttributeSchema> {
    load_schemas(state).unwrap_or_default()
}
