use rusqlite::Connection;

use crate::account::AccountRepo;
use crate::character::CharacterRepo;
use crate::error::PlayerDbError;
use crate::schema;

/// Main database handle wrapping a SQLite connection.
pub struct PlayerDb {
    conn: Connection,
}

impl PlayerDb {
    /// Open (or create) a database at the given file path.
    pub fn open(path: &str) -> Result<Self, PlayerDbError> {
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    PlayerDbError::Database(rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error::new(1),
                        Some(format!("failed to create dir: {}", e)),
                    ))
                })?;
            }
        }

        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        schema::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (for testing).
    pub fn open_memory() -> Result<Self, PlayerDbError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        schema::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// Get account repository.
    pub fn account(&self) -> AccountRepo<'_> {
        AccountRepo::new(&self.conn)
    }

    /// Get character repository.
    pub fn character(&self) -> CharacterRepo<'_> {
        CharacterRepo::new(&self.conn)
    }
}
