use std::collections::HashMap;

use session::SessionId;

use crate::channels::{OutputRx, RegisterRx, SessionWriteTx, UnregisterRx};

/// Routes SessionOutput messages to the correct per-session write channel.
pub async fn run_output_router(
    mut output_rx: OutputRx,
    mut register_rx: RegisterRx,
    mut unregister_rx: UnregisterRx,
) {
    let mut writers: HashMap<SessionId, SessionWriteTx> = HashMap::new();

    loop {
        tokio::select! {
            Some(reg) = register_rx.recv() => {
                tracing::debug!(session_id = ?reg.session_id, "Output router: session registered");
                writers.insert(reg.session_id, reg.write_tx);
            }
            Some(session_id) = unregister_rx.recv() => {
                tracing::debug!(session_id = ?session_id, "Output router: session unregistered");
                writers.remove(&session_id);
            }
            Some(output) = output_rx.recv() => {
                if let Some(tx) = writers.get(&output.session_id) {
                    if tx.send(output.text).is_err() {
                        tracing::debug!(session_id = ?output.session_id, "Output router: session write channel closed");
                        writers.remove(&output.session_id);
                    } else if output.disconnect {
                        tracing::debug!(session_id = ?output.session_id, "Output router: disconnect requested, dropping writer");
                        writers.remove(&output.session_id);
                    }
                }
            }
            else => break,
        }
    }

    tracing::info!("Output router shutting down");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channels::RegisterSession;
    use session::SessionOutput;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn router_delivers_messages() {
        let (output_tx, output_rx) = mpsc::unbounded_channel();
        let (register_tx, register_rx) = mpsc::unbounded_channel();
        let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

        let router_handle = tokio::spawn(run_output_router(output_rx, register_rx, unregister_rx));

        // Register a session
        let (write_tx, mut write_rx) = mpsc::unbounded_channel();
        let sid = SessionId(1);
        register_tx
            .send(RegisterSession {
                session_id: sid,
                write_tx,
            })
            .unwrap();

        // Give the router time to process
        tokio::task::yield_now().await;

        // Send output
        output_tx
            .send(SessionOutput::new(sid, "Hello, player!"))
            .unwrap();

        let msg = write_rx.recv().await.unwrap();
        assert_eq!(msg, "Hello, player!");

        // Unregister
        unregister_tx.send(sid).unwrap();
        tokio::task::yield_now().await;

        // After unregister, messages should be silently dropped
        output_tx
            .send(SessionOutput::new(sid, "Should be dropped"))
            .unwrap();
        tokio::task::yield_now().await;

        // Shutdown
        drop(output_tx);
        drop(register_tx);
        drop(unregister_tx);
        let _ = router_handle.await;
    }
}
