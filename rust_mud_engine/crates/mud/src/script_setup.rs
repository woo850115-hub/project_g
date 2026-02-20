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
}
