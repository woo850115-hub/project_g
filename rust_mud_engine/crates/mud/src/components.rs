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
}
