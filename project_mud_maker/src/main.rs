mod api;
mod config;
mod state;

use config::MudMakerConfig;
use state::AppState;

use axum::Router;
use std::path::Path;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,project_mud_maker=debug".into()),
        )
        .init();

    // Parse CLI args for --config
    let args: Vec<String> = std::env::args().collect();
    let config_path = args
        .windows(2)
        .find(|w| w[0] == "--config")
        .map(|w| w[1].as_str())
        .unwrap_or("project_mud_maker/server.toml");

    let config = if Path::new(config_path).exists() {
        match MudMakerConfig::load(Path::new(config_path)) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to load config from {config_path}: {e}");
                std::process::exit(1);
            }
        }
    } else {
        tracing::info!("Config file not found at {config_path}, using defaults");
        toml::from_str("").unwrap()
    };

    // Ensure content directory exists
    let content_dir = config.content_dir();
    if !content_dir.exists() {
        std::fs::create_dir_all(&content_dir).expect("Failed to create content directory");
        tracing::info!("Created content directory: {}", content_dir.display());
    }

    let addr = config.server.addr.clone();
    let static_dir = config.server.web_static_dir.clone();
    let state = AppState::new(config);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(api::router())
        .fallback_service(ServeDir::new(&static_dir))
        .layer(cors)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("MUD Game Maker listening on http://{addr}");
    tracing::info!("Static files: {static_dir}");

    axum::serve(listener, app).await.unwrap();
}
