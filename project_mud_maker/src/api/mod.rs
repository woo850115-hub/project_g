pub mod triggers;
pub mod world;
pub mod ws;

use axum::{routing::get, Router};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/api/content", maker_common::content::router())
        .nest("/api/scripts", maker_common::scripts::router())
        .nest("/api/server", maker_common::process::router())
        .nest("/api/world", world::router())
        .nest("/api/triggers", triggers::router())
        .route("/ws/logs", get(ws::ws_logs))
        .route("/ws/preview", get(ws::ws_preview))
}
