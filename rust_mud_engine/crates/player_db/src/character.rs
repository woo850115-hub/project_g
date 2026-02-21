use rusqlite::Connection;
use serde_json::Value;

use crate::error::PlayerDbError;

/// A character record from the database.
#[derive(Debug, Clone)]
pub struct CharacterRecord {
    pub id: i64,
    pub account_id: i64,
    pub name: String,
    pub components: Value,
    pub room_id: Option<u64>,
    pub position_x: Option<i32>,
    pub position_y: Option<i32>,
    pub created_at: String,
    pub last_played: Option<String>,
}

/// Repository for character operations.
pub struct CharacterRepo<'a> {
    conn: &'a Connection,
}

impl<'a> CharacterRepo<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Create a new character for an account.
    pub fn create(
        &self,
        account_id: i64,
        name: &str,
        default_components: &Value,
    ) -> Result<CharacterRecord, PlayerDbError> {
        // Check name uniqueness
        if self.get_by_name(name)?.is_some() {
            return Err(PlayerDbError::CharacterNameTaken(name.to_string()));
        }

        let components_str = serde_json::to_string(default_components)
            .unwrap_or_else(|_| "{}".to_string());

        self.conn.execute(
            "INSERT INTO characters (account_id, name, components) VALUES (?1, ?2, ?3)",
            rusqlite::params![account_id, name, components_str],
        )?;

        let id = self.conn.last_insert_rowid();

        Ok(CharacterRecord {
            id,
            account_id,
            name: name.to_string(),
            components: default_components.clone(),
            room_id: None,
            position_x: None,
            position_y: None,
            created_at: String::new(),
            last_played: None,
        })
    }

    /// List all characters for an account.
    pub fn list_for_account(&self, account_id: i64) -> Result<Vec<CharacterRecord>, PlayerDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, components, room_id, position_x, position_y, created_at, last_played
             FROM characters WHERE account_id = ?1 ORDER BY id",
        )?;

        let records = stmt
            .query_map(rusqlite::params![account_id], |row| {
                let components_str: String = row.get(3)?;
                Ok(CharacterRecord {
                    id: row.get(0)?,
                    account_id: row.get(1)?,
                    name: row.get(2)?,
                    components: serde_json::from_str(&components_str).unwrap_or(Value::Object(Default::default())),
                    room_id: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                    position_x: row.get(5)?,
                    position_y: row.get(6)?,
                    created_at: row.get(7)?,
                    last_played: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Load a character by ID.
    pub fn load(&self, id: i64) -> Result<CharacterRecord, PlayerDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, components, room_id, position_x, position_y, created_at, last_played
             FROM characters WHERE id = ?1",
        )?;

        stmt.query_row(rusqlite::params![id], |row| {
            let components_str: String = row.get(3)?;
            Ok(CharacterRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                name: row.get(2)?,
                components: serde_json::from_str(&components_str).unwrap_or(Value::Object(Default::default())),
                room_id: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                position_x: row.get(5)?,
                position_y: row.get(6)?,
                created_at: row.get(7)?,
                last_played: row.get(8)?,
            })
        })
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => PlayerDbError::CharacterNotFound(id),
            other => other.into(),
        })
    }

    /// Save character state (components JSON, position).
    pub fn save_state(
        &self,
        id: i64,
        components: &Value,
        room_id: Option<u64>,
        pos: Option<(i32, i32)>,
    ) -> Result<(), PlayerDbError> {
        let components_str = serde_json::to_string(components)
            .unwrap_or_else(|_| "{}".to_string());
        let room_id_val = room_id.map(|v| v as i64);
        let (px, py) = match pos {
            Some((x, y)) => (Some(x), Some(y)),
            None => (None, None),
        };

        let rows = self.conn.execute(
            "UPDATE characters SET components = ?1, room_id = ?2, position_x = ?3, position_y = ?4, last_played = datetime('now') WHERE id = ?5",
            rusqlite::params![components_str, room_id_val, px, py, id],
        )?;

        if rows == 0 {
            return Err(PlayerDbError::CharacterNotFound(id));
        }
        Ok(())
    }

    /// Delete a character by ID.
    pub fn delete(&self, id: i64) -> Result<(), PlayerDbError> {
        let rows = self.conn.execute(
            "DELETE FROM characters WHERE id = ?1",
            rusqlite::params![id],
        )?;
        if rows == 0 {
            return Err(PlayerDbError::CharacterNotFound(id));
        }
        Ok(())
    }

    /// Get a character by name (case-insensitive).
    pub fn get_by_name(&self, name: &str) -> Result<Option<CharacterRecord>, PlayerDbError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, account_id, name, components, room_id, position_x, position_y, created_at, last_played
             FROM characters WHERE name = ?1",
        )?;

        match stmt.query_row(rusqlite::params![name], |row| {
            let components_str: String = row.get(3)?;
            Ok(CharacterRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                name: row.get(2)?,
                components: serde_json::from_str(&components_str).unwrap_or(Value::Object(Default::default())),
                room_id: row.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                position_x: row.get(5)?,
                position_y: row.get(6)?,
                created_at: row.get(7)?,
                last_played: row.get(8)?,
            })
        }) {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
