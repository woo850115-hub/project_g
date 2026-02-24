use ecs_adapter::Component;
use serde::{Deserialize, Serialize};

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Name(pub String);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Description(pub String);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attack(pub i32);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Defense(pub i32);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub items: Vec<ecs_adapter::EntityId>,
}

impl Inventory {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Component, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PlayerTag;

#[derive(Component, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NpcTag;

#[derive(Component, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ItemTag;

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InRoom(pub ecs_adapter::EntityId);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CombatTarget(pub ecs_adapter::EntityId);

#[derive(Component, Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Dead;

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Race(pub String);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Class(pub String);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Level(pub i32);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mana {
    pub current: i32,
    pub max: i32,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Experience(pub i64);

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CharacterPosition {
    Standing,
    Sitting,
    Resting,
    Sleeping,
    Fighting,
    Incapacitated,
}

impl Default for CharacterPosition {
    fn default() -> Self {
        Self::Standing
    }
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skills {
    pub learned: Vec<String>,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Gold(pub i64);

/// Generic ECS component holding arbitrary JSON data.
/// Custom Serialize/Deserialize implementation to work with bincode:
/// bincode stores the JSON as a string, then deserializes back.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct GameData(pub serde_json::Value);

impl Serialize for GameData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json_str = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_str)
    }
}

impl<'de> Deserialize<'de> for GameData {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let value: serde_json::Value =
            serde_json::from_str(&s).map_err(serde::de::Error::custom)?;
        Ok(GameData(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_bincode_roundtrip() {
        let name = Name("Genos".to_string());
        let bytes = bincode::serialize(&name).unwrap();
        let decoded: Name = bincode::deserialize(&bytes).unwrap();
        assert_eq!(name, decoded);
    }

    #[test]
    fn health_bincode_roundtrip() {
        let hp = Health { current: 80, max: 100 };
        let bytes = bincode::serialize(&hp).unwrap();
        let decoded: Health = bincode::deserialize(&bytes).unwrap();
        assert_eq!(hp, decoded);
    }

    #[test]
    fn inventory_bincode_roundtrip() {
        let inv = Inventory {
            items: vec![
                ecs_adapter::EntityId::new(1, 0),
                ecs_adapter::EntityId::new(5, 2),
            ],
        };
        let bytes = bincode::serialize(&inv).unwrap();
        let decoded: Inventory = bincode::deserialize(&bytes).unwrap();
        assert_eq!(inv, decoded);
    }

    #[test]
    fn combat_target_bincode_roundtrip() {
        let ct = CombatTarget(ecs_adapter::EntityId::new(42, 1));
        let bytes = bincode::serialize(&ct).unwrap();
        let decoded: CombatTarget = bincode::deserialize(&bytes).unwrap();
        assert_eq!(ct, decoded);
    }

    #[test]
    fn dead_bincode_roundtrip() {
        let d = Dead;
        let bytes = bincode::serialize(&d).unwrap();
        let decoded: Dead = bincode::deserialize(&bytes).unwrap();
        assert_eq!(d, decoded);
    }

    #[test]
    fn race_bincode_roundtrip() {
        let race = Race("엘프".to_string());
        let bytes = bincode::serialize(&race).unwrap();
        let decoded: Race = bincode::deserialize(&bytes).unwrap();
        assert_eq!(race, decoded);
    }

    #[test]
    fn class_bincode_roundtrip() {
        let class = Class("마법사".to_string());
        let bytes = bincode::serialize(&class).unwrap();
        let decoded: Class = bincode::deserialize(&bytes).unwrap();
        assert_eq!(class, decoded);
    }

    #[test]
    fn level_bincode_roundtrip() {
        let level = Level(5);
        let bytes = bincode::serialize(&level).unwrap();
        let decoded: Level = bincode::deserialize(&bytes).unwrap();
        assert_eq!(level, decoded);
    }

    #[test]
    fn mana_bincode_roundtrip() {
        let mana = Mana { current: 50, max: 100 };
        let bytes = bincode::serialize(&mana).unwrap();
        let decoded: Mana = bincode::deserialize(&bytes).unwrap();
        assert_eq!(mana, decoded);
    }

    #[test]
    fn experience_bincode_roundtrip() {
        let exp = Experience(1234);
        let bytes = bincode::serialize(&exp).unwrap();
        let decoded: Experience = bincode::deserialize(&bytes).unwrap();
        assert_eq!(exp, decoded);
    }

    #[test]
    fn character_position_bincode_roundtrip() {
        let pos = CharacterPosition::Fighting;
        let bytes = bincode::serialize(&pos).unwrap();
        let decoded: CharacterPosition = bincode::deserialize(&bytes).unwrap();
        assert_eq!(pos, decoded);
    }

    #[test]
    fn gold_bincode_roundtrip() {
        let gold = Gold(150);
        let bytes = bincode::serialize(&gold).unwrap();
        let decoded: Gold = bincode::deserialize(&bytes).unwrap();
        assert_eq!(gold, decoded);
    }

    #[test]
    fn skills_bincode_roundtrip() {
        let skills = Skills { learned: vec!["강타".to_string(), "화염구".to_string()] };
        let bytes = bincode::serialize(&skills).unwrap();
        let decoded: Skills = bincode::deserialize(&bytes).unwrap();
        assert_eq!(skills, decoded);
    }

    #[test]
    fn game_data_bincode_roundtrip() {
        let data = GameData(serde_json::json!({
            "mp": {"current": 50, "max": 100},
            "exp_reward": 25,
            "is_friendly": false
        }));
        let bytes = bincode::serialize(&data).unwrap();
        let decoded: GameData = bincode::deserialize(&bytes).unwrap();
        assert_eq!(data, decoded);
    }
}
