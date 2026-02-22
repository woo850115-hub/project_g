use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use session::SessionId;
use tower_http::services::{ServeDir, ServeFile};

use crate::channels::{
    NetToTick, PlayerTx, RegisterSession, RegisterTx, SessionWriteRx, UnregisterTx,
};

/// Shared state for the axum WebSocket handler.
#[derive(Clone)]
struct AppState {
    next_session_id: Arc<AtomicU64>,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
}

/// Run the web server with WebSocket upgrade and optional static file serving.
///
/// If `static_dir` is Some, serves files from that directory (SPA fallback to index.html).
/// The `/ws` route always handles WebSocket upgrades.
pub async fn run_web_server(
    addr: String,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
    static_dir: Option<PathBuf>,
) -> Result<(), std::io::Error> {
    run_web_server_inner(addr, player_tx, register_tx, unregister_tx, static_dir, None).await
}

/// Run the web server with optional shutdown receiver.
pub async fn run_web_server_with_shutdown(
    addr: String,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
    static_dir: Option<PathBuf>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<(), std::io::Error> {
    run_web_server_inner(addr, player_tx, register_tx, unregister_tx, static_dir, Some(shutdown_rx)).await
}

async fn run_web_server_inner(
    addr: String,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
    static_dir: Option<PathBuf>,
    shutdown_rx: Option<tokio::sync::watch::Receiver<bool>>,
) -> Result<(), std::io::Error> {
    let state = AppState {
        next_session_id: Arc::new(AtomicU64::new(1_000_000)),
        player_tx,
        register_tx,
        unregister_tx,
    };

    let mut app = Router::new()
        .route("/ws", get(ws_upgrade_handler))
        .with_state(state);

    if let Some(dir) = static_dir {
        let index_path = dir.join("index.html");
        let serve_dir = ServeDir::new(&dir).not_found_service(ServeFile::new(index_path));
        app = app.fallback_service(serve_dir);
        tracing::info!(dir = %dir.display(), "Serving static files");
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Web server listening on {}", addr);

    if let Some(mut rx) = shutdown_rx {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                while !*rx.borrow() {
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
                tracing::info!("Web server shutting down gracefully");
            })
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    } else {
        axum::serve(listener, app)
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }
}

async fn ws_upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection(socket: WebSocket, state: AppState) {
    let session_id = SessionId(state.next_session_id.fetch_add(1, Ordering::Relaxed));
    tracing::info!(?session_id, "New WebSocket connection (axum)");

    let (mut ws_writer, mut ws_reader) = socket.split();

    // Create per-session write channel
    let (write_tx, mut write_rx): (_, SessionWriteRx) = tokio::sync::mpsc::unbounded_channel();

    // Register with output router
    let _ = state.register_tx.send(RegisterSession {
        session_id,
        write_tx,
    });

    // Notify tick thread of new connection
    let _ = state.player_tx.send(NetToTick::NewConnection { session_id });

    // Writer task: forward output_router messages as WS text frames
    let writer_handle = tokio::spawn(async move {
        while let Some(text) = write_rx.recv().await {
            if ws_writer.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Reader loop: parse WS messages and convert to NetToTick
    while let Some(result) = ws_reader.next().await {
        match result {
            Ok(Message::Text(text)) => {
                if let Some(net_msg) =
                    crate::ws_server::handle_ws_message(session_id, &text)
                {
                    let _ = state.player_tx.send(net_msg);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {
                // axum handles pong automatically
            }
            Ok(_) => {} // Ignore binary, pong, etc.
            Err(e) => {
                tracing::debug!(?session_id, "WebSocket read error: {}", e);
                break;
            }
        }
    }

    // Notify tick thread of disconnection
    let _ = state.player_tx.send(NetToTick::Disconnected { session_id });
    let _ = state.unregister_tx.send(session_id);

    writer_handle.abort();
    tracing::info!(?session_id, "WebSocket session ended (axum)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_is_clone() {
        // AppState must be Clone for axum State extractor
        fn assert_clone<T: Clone>() {}
        assert_clone::<AppState>();
    }
}
