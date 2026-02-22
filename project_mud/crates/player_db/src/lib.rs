pub mod account;
pub mod character;
pub mod db;
pub mod error;
mod schema;

pub use account::{Account, AccountRepo, PermissionLevel};
pub use character::CharacterRecord;
pub use db::PlayerDb;
pub use error::PlayerDbError;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn open_memory_db() {
        let db = PlayerDb::open_memory().unwrap();
        // Tables should be created
        let count: i64 = db
            .account()
            .get_by_username("nobody")
            .unwrap()
            .map(|_| 1)
            .unwrap_or(0);
        assert_eq!(count, 0);
    }

    #[test]
    fn create_account() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("TestUser", "password123").unwrap();
        assert_eq!(account.username, "TestUser");
        assert_eq!(account.permission, PermissionLevel::Player);
    }

    #[test]
    fn duplicate_account_rejected() {
        let db = PlayerDb::open_memory().unwrap();
        db.account().create("User1", "pass1").unwrap();
        let result = db.account().create("User1", "pass2");
        assert!(matches!(result, Err(PlayerDbError::AccountExists(_))));
    }

    #[test]
    fn case_insensitive_username() {
        let db = PlayerDb::open_memory().unwrap();
        db.account().create("Alice", "pass").unwrap();
        let result = db.account().create("alice", "pass2");
        assert!(matches!(result, Err(PlayerDbError::AccountExists(_))));
    }

    #[test]
    fn authenticate_success() {
        let db = PlayerDb::open_memory().unwrap();
        db.account().create("Hero", "secret123").unwrap();
        let account = db.account().authenticate("Hero", "secret123").unwrap();
        assert_eq!(account.username, "Hero");
    }

    #[test]
    fn authenticate_wrong_password() {
        let db = PlayerDb::open_memory().unwrap();
        db.account().create("Hero", "secret123").unwrap();
        let result = db.account().authenticate("Hero", "wrongpass");
        assert!(matches!(result, Err(PlayerDbError::InvalidPassword)));
    }

    #[test]
    fn authenticate_nonexistent_account() {
        let db = PlayerDb::open_memory().unwrap();
        let result = db.account().authenticate("Ghost", "pass");
        assert!(matches!(result, Err(PlayerDbError::AccountNotFound(_))));
    }

    #[test]
    fn set_permission() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("Admin", "pass").unwrap();
        db.account()
            .set_permission(account.id, PermissionLevel::Admin)
            .unwrap();
        let loaded = db.account().get_by_username("Admin").unwrap().unwrap();
        assert_eq!(loaded.permission, PermissionLevel::Admin);
    }

    #[test]
    fn create_character() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("Player1", "pass").unwrap();
        let defaults = json!({"Name": "용사", "Health": {"current": 100, "max": 100}});
        let character = db
            .character()
            .create(account.id, "용사", &defaults)
            .unwrap();
        assert_eq!(character.name, "용사");
        assert_eq!(character.account_id, account.id);
    }

    #[test]
    fn duplicate_character_name_rejected() {
        let db = PlayerDb::open_memory().unwrap();
        let a1 = db.account().create("P1", "p").unwrap();
        let a2 = db.account().create("P2", "p").unwrap();
        let defaults = json!({});
        db.character().create(a1.id, "Hero", &defaults).unwrap();
        let result = db.character().create(a2.id, "Hero", &defaults);
        assert!(matches!(result, Err(PlayerDbError::CharacterNameTaken(_))));
    }

    #[test]
    fn list_characters_for_account() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("Multi", "pass").unwrap();
        let defaults = json!({});
        db.character()
            .create(account.id, "Char1", &defaults)
            .unwrap();
        db.character()
            .create(account.id, "Char2", &defaults)
            .unwrap();

        let chars = db.character().list_for_account(account.id).unwrap();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].name, "Char1");
        assert_eq!(chars[1].name, "Char2");
    }

    #[test]
    fn save_and_load_character_state() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("Saver", "pass").unwrap();
        let defaults = json!({"Health": {"current": 100, "max": 100}});
        let character = db
            .character()
            .create(account.id, "SaveHero", &defaults)
            .unwrap();

        // Save updated state
        let updated = json!({"Health": {"current": 85, "max": 100}, "Attack": 15});
        db.character()
            .save_state(character.id, &updated, Some(42), None)
            .unwrap();

        // Load and verify
        let loaded = db.character().load(character.id).unwrap();
        assert_eq!(loaded.components["Health"]["current"], 85);
        assert_eq!(loaded.components["Attack"], 15);
        assert_eq!(loaded.room_id, Some(42));
        assert!(loaded.last_played.is_some());
    }

    #[test]
    fn save_character_with_grid_position() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("GridPlayer", "pass").unwrap();
        let character = db
            .character()
            .create(account.id, "GridHero", &json!({}))
            .unwrap();

        db.character()
            .save_state(character.id, &json!({}), None, Some((128, 256)))
            .unwrap();

        let loaded = db.character().load(character.id).unwrap();
        assert_eq!(loaded.position_x, Some(128));
        assert_eq!(loaded.position_y, Some(256));
    }

    #[test]
    fn delete_character() {
        let db = PlayerDb::open_memory().unwrap();
        let account = db.account().create("Deleter", "pass").unwrap();
        let character = db
            .character()
            .create(account.id, "Doomed", &json!({}))
            .unwrap();

        db.character().delete(character.id).unwrap();
        let result = db.character().load(character.id);
        assert!(matches!(result, Err(PlayerDbError::CharacterNotFound(_))));
    }

    #[test]
    fn permission_level_ordering() {
        assert!(PermissionLevel::Player < PermissionLevel::Builder);
        assert!(PermissionLevel::Builder < PermissionLevel::Admin);
        assert!(PermissionLevel::Admin < PermissionLevel::Owner);
    }

    #[test]
    fn permission_level_roundtrip() {
        for level in [
            PermissionLevel::Player,
            PermissionLevel::Builder,
            PermissionLevel::Admin,
            PermissionLevel::Owner,
        ] {
            assert_eq!(PermissionLevel::from_i32(level.as_i32()), level);
        }
    }
}
