use session::{SessionId, SessionOutput};
use tokio::sync::mpsc;

/// Messages from the network layer to the tick thread.
#[derive(Debug)]
pub enum NetToTick {
    /// A new TCP connection was accepted.
    NewConnection {
        session_id: SessionId,
    },
    /// Player typed a line of input.
    PlayerInput {
        session_id: SessionId,
        line: String,
    },
    /// Player disconnected.
    Disconnected {
        session_id: SessionId,
    },
}

/// Sender from network tasks to the tick thread.
pub type PlayerTx = mpsc::UnboundedSender<NetToTick>;
/// Receiver in the tick thread for player events.
pub type PlayerRx = mpsc::UnboundedReceiver<NetToTick>;

/// Sender from tick thread to the output router.
pub type OutputTx = mpsc::UnboundedSender<SessionOutput>;
/// Receiver in the output router for session outputs.
pub type OutputRx = mpsc::UnboundedReceiver<SessionOutput>;

/// Per-session write channel (tick thread -> output router -> session task).
pub type SessionWriteTx = mpsc::UnboundedSender<String>;
pub type SessionWriteRx = mpsc::UnboundedReceiver<String>;

/// Registration message for the output router.
#[derive(Debug)]
pub struct RegisterSession {
    pub session_id: SessionId,
    pub write_tx: SessionWriteTx,
}

pub type RegisterTx = mpsc::UnboundedSender<RegisterSession>;
pub type RegisterRx = mpsc::UnboundedReceiver<RegisterSession>;

pub type UnregisterTx = mpsc::UnboundedSender<SessionId>;
pub type UnregisterRx = mpsc::UnboundedReceiver<SessionId>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn channel_roundtrip() {
        let (tx, mut rx) = mpsc::unbounded_channel::<NetToTick>();

        tx.send(NetToTick::NewConnection {
            session_id: SessionId(1),
        })
        .unwrap();

        tx.send(NetToTick::PlayerInput {
            session_id: SessionId(1),
            line: "north".to_string(),
        })
        .unwrap();

        tx.send(NetToTick::Disconnected {
            session_id: SessionId(1),
        })
        .unwrap();

        let msg1 = rx.recv().await.unwrap();
        assert!(matches!(msg1, NetToTick::NewConnection { .. }));

        let msg2 = rx.recv().await.unwrap();
        assert!(matches!(msg2, NetToTick::PlayerInput { .. }));

        let msg3 = rx.recv().await.unwrap();
        assert!(matches!(msg3, NetToTick::Disconnected { .. }));
    }

    #[tokio::test]
    async fn output_channel_roundtrip() {
        let (tx, mut rx) = mpsc::unbounded_channel::<SessionOutput>();

        tx.send(SessionOutput::new(SessionId(42), "Hello!"))
            .unwrap();

        let msg = rx.recv().await.unwrap();
        assert_eq!(msg.session_id, SessionId(42));
        assert_eq!(msg.text, "Hello!");
    }
}
