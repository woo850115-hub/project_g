use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use maker_common::content::ContentDir;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelEntry {
    pub level: i32,
    pub exp_required: i64,
    pub hp_bonus: i32,
    pub mp_bonus: i32,
    pub atk_bonus: i32,
    pub def_bonus: i32,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(get_level_table).put(save_level_table))
}

/// GET /api/level-table
async fn get_level_table(State(state): State<AppState>) -> impl IntoResponse {
    match load_level_table(&state) {
        Ok(table) => (StatusCode::OK, Json(serde_json::to_value(table).unwrap())),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

/// PUT /api/level-table
async fn save_level_table(
    State(state): State<AppState>,
    Json(table): Json<Vec<LevelEntry>>,
) -> impl IntoResponse {
    let path = level_table_path(&state);

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json = match serde_json::to_string_pretty(&table) {
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

fn level_table_path(state: &AppState) -> std::path::PathBuf {
    state.content_dir().join("level_table.json")
}

fn load_level_table(state: &AppState) -> Result<Vec<LevelEntry>, String> {
    let path = level_table_path(state);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Read error: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Parse error: {e}"))
}

/// Public sync loader for use in world.rs Lua generation.
pub fn load_level_table_sync(state: &AppState) -> Vec<LevelEntry> {
    load_level_table(state).unwrap_or_default()
}
