use ecs_adapter::{Component, EcsAdapter, EntityId};
use scripting::component_registry::{ScriptComponent, ScriptComponentRegistry};
use scripting::error::ScriptError;
use scripting::mlua;
use scripting::mlua::{Lua, LuaSerdeExt};
use serde::{de::DeserializeOwned, Serialize};

use crate::components::*;

/// Generic handler for any Component that implements Serialize + DeserializeOwned.
/// Converts between Rust components and Lua values via serde_json.
struct JsonComponentHandler<C> {
    tag: &'static str,
    _marker: std::marker::PhantomData<C>,
}

impl<C> JsonComponentHandler<C> {
    fn new(tag: &'static str) -> Self {
        Self {
            tag,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C> ScriptComponent for JsonComponentHandler<C>
where
    C: Component + Serialize + DeserializeOwned + Send + Sync,
{
    fn tag(&self) -> &str {
        self.tag
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<C>(eid) {
            Ok(c) => {
                let json_val = serde_json::to_value(c)
                    .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
                let lua_val = lua.to_value(&json_val).map_err(ScriptError::Lua)?;
                Ok(Some(lua_val))
            }
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        lua: &Lua,
    ) -> Result<(), ScriptError> {
        let json_val: serde_json::Value = lua.from_value(value).map_err(ScriptError::Lua)?;
        let component: C = serde_json::from_value(json_val)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        ecs.set_component(eid, component)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<C>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<C>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<C>()
    }
}

fn register<C>(registry: &mut ScriptComponentRegistry, tag: &'static str)
where
    C: Component + Serialize + DeserializeOwned + Send + Sync,
{
    registry.register(Box::new(JsonComponentHandler::<C>::new(tag)));
}

/// Handler for tag (unit struct) components like PlayerTag, NpcTag, ItemTag, Dead.
/// get_as_lua returns true if present (instead of null from JSON serialization).
/// set_from_lua accepts any truthy value and inserts the Default component.
struct TagComponentHandler<C> {
    tag: &'static str,
    _marker: std::marker::PhantomData<C>,
}

impl<C> TagComponentHandler<C> {
    fn new(tag: &'static str) -> Self {
        Self {
            tag,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C> ScriptComponent for TagComponentHandler<C>
where
    C: Component + Default + Send + Sync,
{
    fn tag(&self) -> &str {
        self.tag
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        _lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        if ecs.has_component::<C>(eid) {
            Ok(Some(mlua::Value::Boolean(true)))
        } else {
            Ok(None)
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        // Reject falsy values — use ecs:remove() to unset a tag component.
        if matches!(value, mlua::Value::Nil | mlua::Value::Boolean(false)) {
            return Err(ScriptError::Lua(mlua::Error::runtime(format!(
                "Tag component '{}' requires a truthy value (use ecs:remove to remove)",
                self.tag
            ))));
        }
        ecs.set_component(eid, C::default())
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<C>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<C>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<C>()
    }
}

fn register_tag<C>(registry: &mut ScriptComponentRegistry, tag: &'static str)
where
    C: Component + Default + Send + Sync,
{
    registry.register(Box::new(TagComponentHandler::<C>::new(tag)));
}

/// Handler for CombatTarget(EntityId) — Lua sees/sets a u64.
struct CombatTargetHandler;

impl ScriptComponent for CombatTargetHandler {
    fn tag(&self) -> &str {
        "CombatTarget"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        _lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<CombatTarget>(eid) {
            Ok(ct) => Ok(Some(mlua::Value::Number(ct.0.to_u64() as f64))),
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        let target_u64 = match value {
            mlua::Value::Number(n) => n as u64,
            mlua::Value::Integer(n) => n as u64,
            _ => return Err(ScriptError::Lua(mlua::Error::runtime("CombatTarget expects entity id (number)"))),
        };
        ecs.set_component(eid, CombatTarget(EntityId::from_u64(target_u64)))
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<CombatTarget>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<CombatTarget>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<CombatTarget>()
    }
}

/// Handler for InRoom(EntityId) — Lua sees/sets a u64.
struct InRoomHandler;

impl ScriptComponent for InRoomHandler {
    fn tag(&self) -> &str {
        "InRoom"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        _lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<InRoom>(eid) {
            Ok(ir) => Ok(Some(mlua::Value::Number(ir.0.to_u64() as f64))),
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        let room_u64 = match value {
            mlua::Value::Number(n) => n as u64,
            mlua::Value::Integer(n) => n as u64,
            _ => return Err(ScriptError::Lua(mlua::Error::runtime("InRoom expects entity id (number)"))),
        };
        ecs.set_component(eid, InRoom(EntityId::from_u64(room_u64)))
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<InRoom>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<InRoom>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<InRoom>()
    }
}

/// Handler for Inventory { items: Vec<EntityId> } — Lua sees/sets {items = [u64, ...]}.
struct InventoryHandler;

impl ScriptComponent for InventoryHandler {
    fn tag(&self) -> &str {
        "Inventory"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<Inventory>(eid) {
            Ok(inv) => {
                let table = lua.create_table().map_err(ScriptError::Lua)?;
                let items = lua.create_table().map_err(ScriptError::Lua)?;
                for (i, &item_id) in inv.items.iter().enumerate() {
                    items
                        .set(i + 1, item_id.to_u64())
                        .map_err(ScriptError::Lua)?;
                }
                table.set("items", items).map_err(ScriptError::Lua)?;
                Ok(Some(mlua::Value::Table(table)))
            }
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        let table = match value {
            mlua::Value::Table(t) => t,
            _ => return Err(ScriptError::Lua(mlua::Error::runtime("Inventory expects a table with items field"))),
        };
        let items_table: mlua::Table = table
            .get("items")
            .map_err(ScriptError::Lua)?;
        let mut items = Vec::new();
        for pair in items_table.sequence_values::<u64>() {
            let id = pair.map_err(ScriptError::Lua)?;
            items.push(EntityId::from_u64(id));
        }
        ecs.set_component(eid, Inventory { items })
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<Inventory>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<Inventory>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<Inventory>()
    }
}

/// Handler for Skills { learned: Vec<String> } — explicitly handles sequence conversion.
struct SkillsHandler;

impl ScriptComponent for SkillsHandler {
    fn tag(&self) -> &str {
        "Skills"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<Skills>(eid) {
            Ok(skills) => {
                let table = lua.create_table().map_err(ScriptError::Lua)?;
                let learned = lua.create_table().map_err(ScriptError::Lua)?;
                for (i, skill_name) in skills.learned.iter().enumerate() {
                    learned
                        .set(i + 1, skill_name.as_str())
                        .map_err(ScriptError::Lua)?;
                }
                table.set("learned", learned).map_err(ScriptError::Lua)?;
                Ok(Some(mlua::Value::Table(table)))
            }
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        let table = match value {
            mlua::Value::Table(t) => t,
            _ => {
                return Err(ScriptError::Lua(mlua::Error::runtime(
                    "Skills expects a table with learned field",
                )))
            }
        };
        let learned_table: mlua::Table =
            table.get("learned").map_err(ScriptError::Lua)?;
        let mut learned = Vec::new();
        for pair in learned_table.sequence_values::<String>() {
            let name = pair.map_err(ScriptError::Lua)?;
            learned.push(name);
        }
        ecs.set_component(eid, Skills { learned })
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<Skills>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<Skills>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<Skills>()
    }
}

/// Handler for CharacterPosition enum — Lua sees/sets a lowercase string ("standing", "sitting", etc.)
struct CharacterPositionHandler;

impl ScriptComponent for CharacterPositionHandler {
    fn tag(&self) -> &str {
        "Position"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        _lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<CharacterPosition>(eid) {
            Ok(pos) => {
                let s = match pos {
                    CharacterPosition::Standing => "standing",
                    CharacterPosition::Sitting => "sitting",
                    CharacterPosition::Resting => "resting",
                    CharacterPosition::Sleeping => "sleeping",
                    CharacterPosition::Fighting => "fighting",
                    CharacterPosition::Incapacitated => "incapacitated",
                };
                Ok(Some(mlua::Value::String(
                    _lua.create_string(s).map_err(ScriptError::Lua)?,
                )))
            }
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        _lua: &Lua,
    ) -> Result<(), ScriptError> {
        let s: String = match value {
            mlua::Value::String(s) => s.to_str().map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?.to_string(),
            _ => return Err(ScriptError::Lua(mlua::Error::runtime(
                "Position expects a string (standing/sitting/resting/sleeping/fighting/incapacitated)",
            ))),
        };
        let pos = match s.as_str() {
            "standing" => CharacterPosition::Standing,
            "sitting" => CharacterPosition::Sitting,
            "resting" => CharacterPosition::Resting,
            "sleeping" => CharacterPosition::Sleeping,
            "fighting" => CharacterPosition::Fighting,
            "incapacitated" => CharacterPosition::Incapacitated,
            other => return Err(ScriptError::Lua(mlua::Error::runtime(format!(
                "Unknown position: '{}'. Valid: standing, sitting, resting, sleeping, fighting, incapacitated",
                other
            )))),
        };
        ecs.set_component(eid, pos)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<CharacterPosition>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<CharacterPosition>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<CharacterPosition>()
    }
}

/// Register all MUD component types with the script component registry.
pub fn register_mud_script_components(registry: &mut ScriptComponentRegistry) {
    register::<Name>(registry, "Name");
    register::<Description>(registry, "Description");
    register::<Health>(registry, "Health");
    register::<Attack>(registry, "Attack");
    register::<Defense>(registry, "Defense");
    registry.register(Box::new(InventoryHandler));
    register_tag::<PlayerTag>(registry, "PlayerTag");
    register_tag::<NpcTag>(registry, "NpcTag");
    register_tag::<ItemTag>(registry, "ItemTag");
    registry.register(Box::new(InRoomHandler));
    registry.register(Box::new(CombatTargetHandler));
    register_tag::<Dead>(registry, "Dead");
    register::<Race>(registry, "Race");
    register::<Class>(registry, "Class");
    register::<Level>(registry, "Level");
    register::<Mana>(registry, "Mana");
    register::<Experience>(registry, "Experience");
    registry.register(Box::new(CharacterPositionHandler));
    registry.register(Box::new(SkillsHandler));
    register::<Gold>(registry, "Gold");
    registry.register(Box::new(GameDataHandler));
}

/// Handler for GameData(serde_json::Value) — directly passes JSON value without
/// going through GameData's custom Serialize (which converts to string for bincode).
struct GameDataHandler;

impl ScriptComponent for GameDataHandler {
    fn tag(&self) -> &str {
        "GameData"
    }

    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError> {
        match ecs.get_component::<GameData>(eid) {
            Ok(gd) => {
                let lua_val = lua.to_value(&gd.0).map_err(ScriptError::Lua)?;
                Ok(Some(lua_val))
            }
            Err(_) => Ok(None),
        }
    }

    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        lua: &Lua,
    ) -> Result<(), ScriptError> {
        let json_val: serde_json::Value = lua.from_value(value).map_err(ScriptError::Lua)?;
        ecs.set_component(eid, GameData(json_val))
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
        ecs.has_component::<GameData>(eid)
    }

    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
        ecs.remove_component::<GameData>(eid)
            .map_err(|e| ScriptError::Lua(mlua::Error::runtime(e.to_string())))?;
        Ok(())
    }

    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
        ecs.entities_with::<GameData>()
    }
}
