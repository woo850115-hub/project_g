use axum::Router;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/api/content", maker_common::content::router())
        .nest("/api/scripts", maker_common::scripts::router())
        .nest("/api/server", maker_common::process::router())
}
