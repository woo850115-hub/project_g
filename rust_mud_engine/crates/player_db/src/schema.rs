use rusqlite::Connection;

use crate::error::PlayerDbError;

pub fn create_tables(conn: &Connection) -> Result<(), PlayerDbError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS accounts (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            username      TEXT NOT NULL UNIQUE COLLATE NOCASE,
            password_hash TEXT NOT NULL,
            permission    INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            last_login    TEXT
        );

        CREATE TABLE IF NOT EXISTS characters (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id  INTEGER NOT NULL REFERENCES accounts(id),
            name        TEXT NOT NULL UNIQUE COLLATE NOCASE,
            components  TEXT NOT NULL DEFAULT '{}',
            room_id     INTEGER,
            position_x  INTEGER,
            position_y  INTEGER,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            last_played TEXT
        );
        ",
    )?;
    Ok(())
}
