use std::io;

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("snapshot version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },

    #[error("corrupt snapshot: {0}")]
    Corrupt(String),
}

impl From<bincode::Error> for PersistenceError {
    fn from(e: bincode::Error) -> Self {
        PersistenceError::Serialization(e.to_string())
    }
}
