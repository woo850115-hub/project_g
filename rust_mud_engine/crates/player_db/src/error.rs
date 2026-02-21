use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlayerDbError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("account already exists: {0}")]
    AccountExists(String),

    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("invalid password")]
    InvalidPassword,

    #[error("character name already taken: {0}")]
    CharacterNameTaken(String),

    #[error("character not found: {0}")]
    CharacterNotFound(i64),

    #[error("password hashing error: {0}")]
    HashError(String),
}
