use player_db::PlayerDb;
use scripting::auth::{
    AuthAccountInfo, AuthCharacterDetail, AuthCharacterSummary, AuthError, AuthProvider,
};

/// Wraps PlayerDb to implement the engine's AuthProvider trait.
pub struct PlayerDbAuthProvider<'a> {
    db: &'a PlayerDb,
}

impl<'a> PlayerDbAuthProvider<'a> {
    pub fn new(db: &'a PlayerDb) -> Self {
        Self { db }
    }
}

fn map_err(e: player_db::PlayerDbError) -> AuthError {
    match e {
        player_db::PlayerDbError::AccountNotFound(u) => AuthError::AccountNotFound(u),
        player_db::PlayerDbError::AccountExists(u) => AuthError::AccountExists(u),
        player_db::PlayerDbError::InvalidPassword => AuthError::InvalidPassword,
        player_db::PlayerDbError::CharacterNotFound(id) => AuthError::CharacterNotFound(id),
        player_db::PlayerDbError::CharacterNameTaken(n) => AuthError::CharacterNameTaken(n),
        other => AuthError::Internal(other.to_string()),
    }
}

impl AuthProvider for PlayerDbAuthProvider<'_> {
    fn check_account(&self, username: &str) -> Result<Option<AuthAccountInfo>, AuthError> {
        match self.db.account().get_by_username(username) {
            Ok(Some(account)) => Ok(Some(AuthAccountInfo {
                id: account.id,
                username: account.username,
                permission: account.permission.as_i32(),
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn authenticate(&self, username: &str, password: &str) -> Result<AuthAccountInfo, AuthError> {
        let account = self.db.account().authenticate(username, password).map_err(map_err)?;
        Ok(AuthAccountInfo {
            id: account.id,
            username: account.username,
            permission: account.permission.as_i32(),
        })
    }

    fn create_account(&self, username: &str, password: &str) -> Result<AuthAccountInfo, AuthError> {
        let account = self.db.account().create(username, password).map_err(map_err)?;
        Ok(AuthAccountInfo {
            id: account.id,
            username: account.username,
            permission: account.permission.as_i32(),
        })
    }

    fn list_characters(&self, account_id: i64) -> Result<Vec<AuthCharacterSummary>, AuthError> {
        let chars = self
            .db
            .character()
            .list_for_account(account_id)
            .map_err(map_err)?;
        Ok(chars
            .into_iter()
            .map(|c| AuthCharacterSummary {
                id: c.id,
                name: c.name,
            })
            .collect())
    }

    fn create_character(
        &self,
        account_id: i64,
        name: &str,
        defaults: &serde_json::Value,
    ) -> Result<AuthCharacterDetail, AuthError> {
        let c = self
            .db
            .character()
            .create(account_id, name, defaults)
            .map_err(map_err)?;
        Ok(AuthCharacterDetail {
            id: c.id,
            account_id: c.account_id,
            name: c.name,
            components: c.components,
            room_id: c.room_id,
            position_x: c.position_x,
            position_y: c.position_y,
        })
    }

    fn load_character(&self, character_id: i64) -> Result<AuthCharacterDetail, AuthError> {
        let c = self.db.character().load(character_id).map_err(map_err)?;
        Ok(AuthCharacterDetail {
            id: c.id,
            account_id: c.account_id,
            name: c.name,
            components: c.components,
            room_id: c.room_id,
            position_x: c.position_x,
            position_y: c.position_y,
        })
    }

    fn save_character(
        &self,
        character_id: i64,
        components: &serde_json::Value,
        room_id: Option<u64>,
        position: Option<(i32, i32)>,
    ) -> Result<(), AuthError> {
        self.db
            .character()
            .save_state(character_id, components, room_id, position)
            .map_err(map_err)
    }
}
