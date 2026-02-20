use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use session::SessionId;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use crate::channels::{
    NetToTick, PlayerTx, RegisterSession, RegisterTx, SessionWriteRx, UnregisterTx,
};
use crate::protocol::ClientMessage;

/// WebSocket session IDs start at 1_000_000 to avoid collision with Telnet sessions.
static NEXT_WS_SESSION_ID: AtomicU64 = AtomicU64::new(1_000_000);

/// Run the WebSocket server, accepting connections and spawning per-session tasks.
pub async fn run_ws_server(
    addr: String,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("WebSocket server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let session_id = SessionId(NEXT_WS_SESSION_ID.fetch_add(1, Ordering::Relaxed));

        tracing::info!(?session_id, %peer_addr, "New WebSocket connection");

        let player_tx = player_tx.clone();
        let register_tx = register_tx.clone();
        let unregister_tx = unregister_tx.clone();

        tokio::spawn(async move {
            match tokio_tungstenite::accept_async(stream).await {
                Ok(ws_stream) => {
                    handle_ws_session(ws_stream, session_id, player_tx, register_tx, unregister_tx)
                        .await;
                }
                Err(e) => {
                    tracing::warn!(?session_id, "WebSocket handshake failed: {}", e);
                }
            }
        });
    }
}

async fn handle_ws_session(
    ws_stream: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
    session_id: SessionId,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
) {
    let (mut ws_writer, mut ws_reader) = ws_stream.split();

    // Create per-session write channel
    let (write_tx, mut write_rx): (_, SessionWriteRx) = tokio::sync::mpsc::unbounded_channel();

    // Register with output router
    let _ = register_tx.send(RegisterSession {
        session_id,
        write_tx,
    });

    // Notify tick thread of new connection
    let _ = player_tx.send(NetToTick::NewConnection { session_id });

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
                if let Some(net_msg) = handle_ws_message(session_id, &text) {
                    let _ = player_tx.send(net_msg);
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) => {
                // tungstenite handles pong automatically
            }
            Ok(_) => {} // Ignore binary, pong, etc.
            Err(e) => {
                tracing::debug!(?session_id, "WebSocket read error: {}", e);
                break;
            }
        }
    }

    // Notify tick thread of disconnection
    let _ = player_tx.send(NetToTick::Disconnected { session_id });
    let _ = unregister_tx.send(session_id);

    writer_handle.abort();
    tracing::info!(?session_id, "WebSocket session ended");
}

/// Parse a WebSocket text message into a NetToTick message.
pub(crate) fn handle_ws_message(session_id: SessionId, text: &str) -> Option<NetToTick> {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!(?session_id, "Invalid client message: {}", e);
            return None;
        }
    };

    match msg {
        ClientMessage::Connect { name } => Some(NetToTick::PlayerInput {
            session_id,
            line: name,
        }),
        ClientMessage::Move { dx, dy } => Some(NetToTick::PlayerInput {
            session_id,
            line: format!("__grid_move {} {}", dx, dy),
        }),
        ClientMessage::Action { name, args } => {
            let line = if let Some(a) = args {
                format!("{} {}", name, a)
            } else {
                name
            };
            Some(NetToTick::PlayerInput { session_id, line })
        }
        ClientMessage::Ping => {
            // Pong is handled at the protocol level by sending a ServerMessage::Pong
            // We encode it as a special command the tick thread can recognize,
            // but for simplicity we handle it inline: no tick thread involvement needed.
            // Instead, we return None and the ws_server could send pong directly.
            // However, our architecture routes everything through output_router,
            // so we use a special line prefix.
            Some(NetToTick::PlayerInput {
                session_id,
                line: "__ping".to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_connect_message() {
        let sid = SessionId(1_000_000);
        let msg = handle_ws_message(sid, r#"{"type":"connect","name":"Alice"}"#);
        match msg {
            Some(NetToTick::PlayerInput { session_id, line }) => {
                assert_eq!(session_id, sid);
                assert_eq!(line, "Alice");
            }
            _ => panic!("Expected PlayerInput"),
        }
    }

    #[test]
    fn handle_move_message() {
        let sid = SessionId(1_000_000);
        let msg = handle_ws_message(sid, r#"{"type":"move","dx":1,"dy":0}"#);
        match msg {
            Some(NetToTick::PlayerInput { session_id, line }) => {
                assert_eq!(session_id, sid);
                assert_eq!(line, "__grid_move 1 0");
            }
            _ => panic!("Expected PlayerInput"),
        }
    }

    #[test]
    fn handle_action_message() {
        let sid = SessionId(1_000_001);
        let msg = handle_ws_message(sid, r#"{"type":"action","name":"attack","args":"goblin"}"#);
        match msg {
            Some(NetToTick::PlayerInput { session_id, line }) => {
                assert_eq!(session_id, sid);
                assert_eq!(line, "attack goblin");
            }
            _ => panic!("Expected PlayerInput"),
        }
    }

    #[test]
    fn handle_ping_message() {
        let sid = SessionId(1_000_000);
        let msg = handle_ws_message(sid, r#"{"type":"ping"}"#);
        match msg {
            Some(NetToTick::PlayerInput { line, .. }) => {
                assert_eq!(line, "__ping");
            }
            _ => panic!("Expected PlayerInput with __ping"),
        }
    }

    #[test]
    fn handle_invalid_json() {
        let sid = SessionId(1_000_000);
        let msg = handle_ws_message(sid, "not json");
        assert!(msg.is_none());
    }
}
