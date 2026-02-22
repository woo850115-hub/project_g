use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::rand_core::OsRng;
use password_hash::SaltString;
use rusqlite::Connection;

use crate::error::PlayerDbError;

/// Permission levels for accounts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum PermissionLevel {
    Player = 0,
    Builder = 1,
    Admin = 2,
    Owner = 3,
}

impl PermissionLevel {
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => Self::Builder,
            2 => Self::Admin,
            3 => Self::Owner,
            _ => Self::Player,
        }
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// An account record.
#[derive(Debug, Clone)]
pub struct Account {
    pub id: i64,
    pub username: String,
    pub permission: PermissionLevel,
    pub created_at: String,
    pub last_login: Option<String>,
}

/// Repository for account operations.
pub struct AccountRepo<'a> {
    conn: &'a Connection,
}

impl<'a> AccountRepo<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new account with the given username and password.
    pub fn create(&self, username: &str, password: &str) -> Result<Account, PlayerDbError> {
        // Check for existing account
        if self.get_by_username(username)?.is_some() {
            return Err(PlayerDbError::AccountExists(username.to_string()));
        }

        let password_hash = hash_password(password)?;

        self.conn.execute(
            "INSERT INTO accounts (username, password_hash) VALUES (?1, ?2)",
            rusqlite::params![username, password_hash],
        )?;

        let id = self.conn.last_insert_rowid();

        Ok(Account {
            id,
            username: username.to_string(),
            permission: PermissionLevel::Player,
            created_at: String::new(), // Will be filled by DB default
            last_login: None,
        })
    }

    /// Authenticate with username and password. Returns the account on success.
    pub fn authenticate(&self, username: &str, password: &str) -> Result<Account, PlayerDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, password_hash, permission, created_at, last_login FROM accounts WHERE username = ?1",
        )?;

        let result = stmt.query_row(rusqlite::params![username], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i32>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
            ))
        });

        let (id, username, password_hash, permission, created_at, last_login) = match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(PlayerDbError::AccountNotFound(username.to_string()));
            }
            Err(e) => return Err(e.into()),
        };

        verify_password(password, &password_hash)?;

        // Update last_login
        self.conn.execute(
            "UPDATE accounts SET last_login = datetime('now') WHERE id = ?1",
            rusqlite::params![id],
        )?;

        Ok(Account {
            id,
            username,
            permission: PermissionLevel::from_i32(permission),
            created_at,
            last_login,
        })
    }

    /// Get an account by username (case-insensitive).
    pub fn get_by_username(&self, username: &str) -> Result<Option<Account>, PlayerDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, username, permission, created_at, last_login FROM accounts WHERE username = ?1",
        )?;

        let result = stmt.query_row(rusqlite::params![username], |row| {
            Ok(Account {
                id: row.get(0)?,
                username: row.get(1)?,
                permission: PermissionLevel::from_i32(row.get(2)?),
                created_at: row.get(3)?,
                last_login: row.get(4)?,
            })
        });

        match result {
            Ok(account) => Ok(Some(account)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set the permission level of an account.
    pub fn set_permission(&self, id: i64, level: PermissionLevel) -> Result<(), PlayerDbError> {
        let rows = self.conn.execute(
            "UPDATE accounts SET permission = ?1 WHERE id = ?2",
            rusqlite::params![level.as_i32(), id],
        )?;
        if rows == 0 {
            return Err(PlayerDbError::AccountNotFound(id.to_string()));
        }
        Ok(())
    }
}

fn hash_password(password: &str) -> Result<String, PlayerDbError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| PlayerDbError::HashError(e.to_string()))
}

fn verify_password(password: &str, hash: &str) -> Result<(), PlayerDbError> {
    let parsed = PasswordHash::new(hash).map_err(|e| PlayerDbError::HashError(e.to_string()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| PlayerDbError::InvalidPassword)
}
