use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use maker_common::process::HasProcessManager;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::state::AppState;

/// WebSocket /ws/logs — stream server stdout/stderr in real time
pub async fn ws_logs(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_logs(socket, state))
}

async fn handle_logs(socket: WebSocket, state: AppState) {
    let (mut sink, _stream) = socket.split();
    let mut rx = state.process_manager().subscribe_logs();

    loop {
        match rx.recv().await {
            Ok(log_line) => {
                let msg = serde_json::json!({"type": "log", "text": log_line.text});
                if sink
                    .send(Message::Text(msg.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("Log subscriber lagged {n} messages");
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

/// WebSocket /ws/preview — telnet proxy (browser WS <-> MUD telnet)
pub async fn ws_preview(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let telnet_addr = state.telnet_addr();
    ws.on_upgrade(move |socket| handle_preview(socket, telnet_addr))
}

async fn handle_preview(socket: WebSocket, telnet_addr: String) {
    // Connect to MUD server telnet
    let tcp = match TcpStream::connect(&telnet_addr).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to connect to MUD server at {telnet_addr}: {e}");
            let (mut sink, _) = socket.split();
            let err = serde_json::json!({"type": "error", "text": format!("Cannot connect to MUD server: {e}")});
            let _ = sink.send(Message::Text(err.to_string().into())).await;
            return;
        }
    };

    let (tcp_read, mut tcp_write) = tcp.into_split();
    let (mut ws_sink, mut ws_stream) = socket.split();

    // TCP -> WS: read from telnet, send to browser
    let tcp_to_ws = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(tcp_read);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    // Filter out telnet negotiation bytes (IAC sequences: 0xFF ...)
                    let text = filter_telnet_bytes(&buf[..n]);
                    if !text.is_empty() {
                        let msg = serde_json::json!({"type": "output", "text": text});
                        if ws_sink
                            .send(Message::Text(msg.to_string().into()))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    // WS -> TCP: receive from browser, write to telnet
    let ws_to_tcp = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(input) = parsed.get("text").and_then(|v| v.as_str()) {
                            let line = format!("{input}\r\n");
                            if tcp_write.write_all(line.as_bytes()).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either direction to finish
    tokio::select! {
        _ = tcp_to_ws => {}
        _ = ws_to_tcp => {}
    }
}

/// Strip telnet IAC negotiation sequences from raw bytes.
fn filter_telnet_bytes(data: &[u8]) -> String {
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i] == 0xFF && i + 2 < data.len() {
            // IAC + command + option: skip 3 bytes
            i += 3;
        } else if data[i] == 0xFF && i + 1 < data.len() {
            // IAC + something: skip 2 bytes
            i += 2;
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).to_string()
}
