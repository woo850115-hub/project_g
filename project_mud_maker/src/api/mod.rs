pub mod attribute_schemas;
pub mod dialogues;
pub mod items;
pub mod quests;
pub mod shops;
pub mod triggers;
pub mod world;
pub mod ws;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/api/content", maker_common::content::router())
        .nest("/api/scripts", maker_common::scripts::router())
        .nest("/api/server", maker_common::process::router())
        .nest("/api/world", world::router())
        .nest("/api/triggers", triggers::router())
        .nest("/api/items", items::router())
        .nest("/api/shops", shops::router())
        .nest("/api/dialogues", dialogues::router())
        .nest("/api/quests", quests::router())
        .nest("/api/attribute-schemas", attribute_schemas::router())
        .route("/api/generate-all", post(generate_all))
        .route("/ws/logs", get(ws::ws_logs))
        .route("/ws/preview", get(ws::ws_preview))
}

async fn generate_all(State(state): State<AppState>) -> impl IntoResponse {
    let mut results = Vec::new();

    // 1. World generate
    let world_path = std::path::PathBuf::from(&state.config.project.mud_dir).join("world.json");
    if world_path.exists() {
        match world::generate_world_lua_inner(&state) {
            Ok(path) => results.push(format!("world: {path}")),
            Err(e) => results.push(format!("world: error - {e}")),
        }
    }

    // 2. Triggers generate
    match triggers::generate_triggers_lua_inner(&state) {
        Ok(path) => results.push(format!("triggers: {path}")),
        Err(e) => results.push(format!("triggers: error - {e}")),
    }

    // 3. Item effects generate
    match items::generate_item_effects_lua_inner(&state) {
        Ok(path) => results.push(format!("items: {path}")),
        Err(e) => results.push(format!("items: error - {e}")),
    }

    // 4. Shops generate
    match shops::generate_shops_lua_inner(&state) {
        Ok(path) => results.push(format!("shops: {path}")),
        Err(e) => results.push(format!("shops: error - {e}")),
    }

    // 5. Dialogues generate
    match dialogues::generate_dialogues_lua_inner(&state) {
        Ok(path) => results.push(format!("dialogues: {path}")),
        Err(e) => results.push(format!("dialogues: error - {e}")),
    }

    // 6. Quests generate
    match quests::generate_quests_lua_inner(&state) {
        Ok(path) => results.push(format!("quests: {path}")),
        Err(e) => results.push(format!("quests: error - {e}")),
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({"ok": true, "results": results})),
    )
}
