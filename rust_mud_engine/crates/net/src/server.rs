use std::sync::atomic::{AtomicU64, Ordering};

use session::SessionId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::channels::{
    NetToTick, PlayerTx, RegisterSession, RegisterTx, SessionWriteRx, UnregisterTx,
};
use crate::telnet::LineBuffer;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(0);

/// Run the TCP server, accepting connections and spawning per-session tasks.
pub async fn run_tcp_server(
    addr: String,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("TCP server listening on {}", addr);

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let session_id = SessionId(NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed));

        tracing::info!(?session_id, %peer_addr, "New connection");

        let player_tx = player_tx.clone();
        let register_tx = register_tx.clone();
        let unregister_tx = unregister_tx.clone();

        tokio::spawn(async move {
            handle_session(stream, session_id, player_tx, register_tx, unregister_tx).await;
        });
    }
}

async fn handle_session(
    stream: tokio::net::TcpStream,
    session_id: SessionId,
    player_tx: PlayerTx,
    register_tx: RegisterTx,
    unregister_tx: UnregisterTx,
) {
    let (mut reader, mut writer) = stream.into_split();

    // Create per-session write channel
    let (write_tx, mut write_rx): (_, SessionWriteRx) =
        tokio::sync::mpsc::unbounded_channel();

    // Register with output router
    let _ = register_tx.send(RegisterSession {
        session_id,
        write_tx,
    });

    // Notify tick thread of new connection
    let _ = player_tx.send(NetToTick::NewConnection { session_id });

    // Spawn writer task
    let writer_handle = tokio::spawn(async move {
        while let Some(text) = write_rx.recv().await {
            // Convert bare \n to \r\n for Telnet clients (e.g. PuTTY)
            let text = text.replace("\r\n", "\n").replace('\n', "\r\n");
            let msg = format!("{}\r\n", text);
            if writer.write_all(msg.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    // Reader loop
    let mut line_buffer = LineBuffer::new();
    let mut buf = [0u8; 4096];

    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break, // Connection closed
            Ok(n) => {
                let lines = line_buffer.feed(&buf[..n]);
                for line in lines {
                    let _ = player_tx.send(NetToTick::PlayerInput {
                        session_id,
                        line,
                    });
                }
            }
            Err(_) => break,
        }
    }

    // Notify tick thread of disconnection
    let _ = player_tx.send(NetToTick::Disconnected { session_id });
    let _ = unregister_tx.send(session_id);

    writer_handle.abort();
    tracing::info!(?session_id, "Session ended");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn server_accepts_connection() {
        let (player_tx, mut player_rx) = mpsc::unbounded_channel();
        let (register_tx, _register_rx) = mpsc::unbounded_channel();
        let (unregister_tx, _unregister_rx) = mpsc::unbounded_channel();

        // Start server on random port
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_handle = tokio::spawn(run_tcp_server(
            addr.to_string(),
            player_tx,
            register_tx,
            unregister_tx,
        ));

        // Small delay for server to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Connect
        let mut stream = TcpStream::connect(addr).await.unwrap();

        // Should receive NewConnection
        let msg = player_rx.recv().await.unwrap();
        assert!(matches!(msg, NetToTick::NewConnection { .. }));

        // Send input
        stream.write_all(b"north\n").await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let msg = player_rx.recv().await.unwrap();
        match msg {
            NetToTick::PlayerInput { line, .. } => assert_eq!(line, "north"),
            _ => panic!("Expected PlayerInput"),
        }

        // Disconnect
        drop(stream);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let msg = player_rx.recv().await.unwrap();
        assert!(matches!(msg, NetToTick::Disconnected { .. }));

        server_handle.abort();
    }

    #[tokio::test]
    async fn server_sends_output() {
        let (player_tx, _player_rx) = mpsc::unbounded_channel();
        let (register_tx, mut register_rx) = mpsc::unbounded_channel();
        let (unregister_tx, _unregister_rx) = mpsc::unbounded_channel();

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        let server_handle = tokio::spawn(run_tcp_server(
            addr.to_string(),
            player_tx,
            register_tx,
            unregister_tx,
        ));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let mut stream = TcpStream::connect(addr).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Get the registered write channel
        let reg = register_rx.recv().await.unwrap();

        // Send text through the write channel
        reg.write_tx.send("Welcome!".to_string()).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Read from client
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        let received = String::from_utf8_lossy(&buf[..n]);
        assert!(received.contains("Welcome!"));

        drop(stream);
        server_handle.abort();
    }
}
