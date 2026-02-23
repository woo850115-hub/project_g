use std::fmt;

/// Account information returned by AuthProvider.
#[derive(Debug, Clone)]
pub struct AuthAccountInfo {
    pub id: i64,
    pub username: String,
    pub permission: i32,
}

/// Summary of a character (for listing).
#[derive(Debug, Clone)]
pub struct AuthCharacterSummary {
    pub id: i64,
    pub name: String,
}

/// Full character detail (for loading into the game).
#[derive(Debug, Clone)]
pub struct AuthCharacterDetail {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub components: serde_json::Value,
    pub room_id: Option<u64>,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
}

/// Errors from auth operations.
#[derive(Debug)]
pub enum AuthError {
    AccountNotFound(String),
    AccountExists(String),
    InvalidPassword,
    CharacterNotFound(i64),
    CharacterNameTaken(String),
    Internal(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::AccountNotFound(u) => write!(f, "account not found: {}", u),
            AuthError::AccountExists(u) => write!(f, "account exists: {}", u),
            AuthError::InvalidPassword => write!(f, "invalid password"),
            AuthError::CharacterNotFound(id) => write!(f, "character not found: {}", id),
            AuthError::CharacterNameTaken(n) => write!(f, "character name taken: {}", n),
            AuthError::Internal(msg) => write!(f, "internal error: {}", msg),
        }
    }
}

/// Trait for authentication and character database operations.
/// Implemented by game layer (e.g., PlayerDbAuthProvider wrapping PlayerDb).
/// Used by Lua AuthProxy to provide login functionality to scripts.
pub trait AuthProvider {
    /// Check if an account exists by username. Returns Some(info) or None.
    fn check_account(&self, username: &str) -> Result<Option<AuthAccountInfo>, AuthError>;

    /// Authenticate with username and password. Returns account info on success.
    fn authenticate(&self, username: &str, password: &str) -> Result<AuthAccountInfo, AuthError>;

    /// Create a new account. Returns account info on success.
    fn create_account(&self, username: &str, password: &str) -> Result<AuthAccountInfo, AuthError>;

    /// List characters for an account.
    fn list_characters(&self, account_id: i64) -> Result<Vec<AuthCharacterSummary>, AuthError>;

    /// Create a new character for an account.
    fn create_character(
        &self,
        account_id: i64,
        name: &str,
        defaults: &serde_json::Value,
    ) -> Result<AuthCharacterDetail, AuthError>;

    /// Load full character detail by ID.
    fn load_character(&self, character_id: i64) -> Result<AuthCharacterDetail, AuthError>;

    /// Save character state to the database.
    fn save_character(
        &self,
        character_id: i64,
        components: &serde_json::Value,
        room_id: Option<u64>,
        position: Option<(i32, i32)>,
    ) -> Result<(), AuthError>;
}
