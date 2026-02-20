use std::cell::RefCell;

use ecs_adapter::{EcsAdapter, EntityId};
use mlua::{Lua, Result as LuaResult, UserData, UserDataMethods, Value};

use crate::component_registry::ScriptComponentRegistry;

/// Proxy object that Lua scripts use to access ECS operations.
/// Wraps a RefCell<&mut EcsAdapter> so that multiple Lua functions
/// can borrow it within the same scope.
pub struct EcsProxy {
    ecs: RefCell<*mut EcsAdapter>,
    registry: *const ScriptComponentRegistry,
}

// SAFETY: EcsProxy is only used within a single tick-thread scope.
// The raw pointers are valid for the duration of the scope.
unsafe impl Send for EcsProxy {}
unsafe impl Sync for EcsProxy {}

impl EcsProxy {
    /// Create a new EcsProxy.
    ///
    /// # Safety
    /// The caller must ensure that `ecs` and `registry` outlive the EcsProxy
    /// and that the proxy is only used from a single thread.
    pub unsafe fn new(ecs: *mut EcsAdapter, registry: *const ScriptComponentRegistry) -> Self {
        Self {
            ecs: RefCell::new(ecs),
            registry,
        }
    }

    fn with_ecs<R>(&self, f: impl FnOnce(&EcsAdapter) -> R) -> R {
        let ptr = *self.ecs.borrow();
        // SAFETY: valid for scope lifetime, single thread
        f(unsafe { &*ptr })
    }

    fn with_ecs_mut<R>(&self, f: impl FnOnce(&mut EcsAdapter) -> R) -> R {
        let ptr = *self.ecs.borrow();
        // SAFETY: valid for scope lifetime, single thread
        f(unsafe { &mut *ptr })
    }

    fn registry(&self) -> &ScriptComponentRegistry {
        // SAFETY: valid for scope lifetime
        unsafe { &*self.registry }
    }
}

impl UserData for EcsProxy {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // ecs:get(entity_id, component_tag) -> value or nil
        methods.add_method("get", |lua, this, (eid_u64, tag): (u64, String)| {
            let eid = EntityId::from_u64(eid_u64);
            let handler = this
                .registry()
                .get(&tag)
                .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", tag)))?;
            let result = this.with_ecs(|ecs| handler.get_as_lua(ecs, eid, lua));
            match result {
                Ok(Some(v)) => Ok(v),
                Ok(None) => Ok(Value::Nil),
                Err(e) => Err(mlua::Error::runtime(e.to_string())),
            }
        });

        // ecs:set(entity_id, component_tag, value)
        methods.add_method("set", |lua, this, (eid_u64, tag, value): (u64, String, Value)| {
            let eid = EntityId::from_u64(eid_u64);
            let handler = this
                .registry()
                .get(&tag)
                .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", tag)))?;
            this.with_ecs_mut(|ecs| handler.set_from_lua(ecs, eid, value, lua))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // ecs:has(entity_id, component_tag) -> bool
        methods.add_method("has", |_lua, this, (eid_u64, tag): (u64, String)| {
            let eid = EntityId::from_u64(eid_u64);
            let handler = this
                .registry()
                .get(&tag)
                .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", tag)))?;
            Ok(this.with_ecs(|ecs| handler.has(ecs, eid)))
        });

        // ecs:remove(entity_id, component_tag)
        methods.add_method("remove", |_lua, this, (eid_u64, tag): (u64, String)| {
            let eid = EntityId::from_u64(eid_u64);
            let handler = this
                .registry()
                .get(&tag)
                .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", tag)))?;
            this.with_ecs_mut(|ecs| handler.remove(ecs, eid))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // ecs:spawn() -> entity_id (u64)
        methods.add_method("spawn", |_lua, this, ()| {
            let eid = this.with_ecs_mut(|ecs| ecs.spawn_entity());
            Ok(eid.to_u64())
        });

        // ecs:despawn(entity_id)
        methods.add_method("despawn", |_lua, this, eid_u64: u64| {
            let eid = EntityId::from_u64(eid_u64);
            this.with_ecs_mut(|ecs| ecs.despawn_entity(eid))
                .map_err(|e| mlua::Error::runtime(e.to_string()))?;
            Ok(())
        });

        // ecs:query(tag1, tag2, ...) -> list of entity_ids
        // Returns entities that have ALL specified components
        methods.add_method("query", |_lua, this, tags: mlua::Variadic<String>| {
            if tags.is_empty() {
                return Err(mlua::Error::runtime("query requires at least one component tag"));
            }

            let registry = this.registry();

            // Get entities for first tag
            let first_tag = &tags[0];
            let first_handler = registry
                .get(first_tag)
                .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", first_tag)))?;

            let mut result = this.with_ecs(|ecs| first_handler.entities_with(ecs));

            // Intersect with remaining tags
            for tag in tags.iter().skip(1) {
                let handler = registry
                    .get(tag)
                    .ok_or_else(|| mlua::Error::runtime(format!("component not registered: {}", tag)))?;
                this.with_ecs(|ecs| {
                    result.retain(|&eid| handler.has(ecs, eid));
                });
            }

            // Convert to u64 list
            let u64s: Vec<u64> = result.iter().map(|e| e.to_u64()).collect();
            Ok(u64s)
        });
    }
}

/// Register the `ecs` global table in Lua using function-style API.
/// This creates thin wrapper functions that delegate to an EcsProxy userdata.
pub fn register_ecs_api(lua: &Lua) -> LuaResult<()> {
    // The actual ecs table will be populated when run_* methods set up the proxy.
    // For now, create an empty placeholder.
    let ecs_table = lua.create_table()?;
    lua.globals().set("ecs", ecs_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_registry::{ScriptComponent, ScriptComponentRegistry};
    use crate::error::ScriptError;
    use crate::sandbox::{ScriptConfig, create_sandboxed_lua};
    use ecs_adapter::{Component, EcsAdapter, EntityId};
    use mlua::LuaSerdeExt;
    use serde::{Deserialize, Serialize};

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Health {
        current: i32,
        max: i32,
    }

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Name(String);

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct PlayerTag;

    /// Generic ScriptComponent handler using serde_json for Lua conversion.
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
        C: Component + Serialize + serde::de::DeserializeOwned + Send + Sync,
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
                    let lua_val = lua
                        .to_value(&json_val)
                        .map_err(ScriptError::Lua)?;
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
            let json_val: serde_json::Value = lua
                .from_value(value)
                .map_err(ScriptError::Lua)?;
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

    fn make_registry() -> ScriptComponentRegistry {
        let mut reg = ScriptComponentRegistry::new();
        reg.register(Box::new(JsonComponentHandler::<Health>::new("Health")));
        reg.register(Box::new(JsonComponentHandler::<Name>::new("Name")));
        reg.register(Box::new(JsonComponentHandler::<PlayerTag>::new("PlayerTag")));
        reg
    }

    #[test]
    fn test_ecs_get_set_roundtrip() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut ecs = EcsAdapter::new();
        let registry = make_registry();

        let e = ecs.spawn_entity();
        ecs.set_component(e, Health { current: 80, max: 100 }).unwrap();

        // Test get
        let proxy = unsafe { EcsProxy::new(&mut ecs as *mut _, &registry as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_ecs", ud).unwrap();

            let result: mlua::Value = lua.load(&format!(
                "return _ecs:get({}, 'Health')", e.to_u64()
            )).eval().unwrap();

            if let Value::Table(t) = result {
                let current: i32 = t.get("current").unwrap();
                let max: i32 = t.get("max").unwrap();
                assert_eq!(current, 80);
                assert_eq!(max, 100);
            } else {
                panic!("Expected table, got {:?}", result);
            }

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_ecs_has() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut ecs = EcsAdapter::new();
        let registry = make_registry();

        let e = ecs.spawn_entity();
        ecs.set_component(e, Health { current: 80, max: 100 }).unwrap();

        let proxy = unsafe { EcsProxy::new(&mut ecs as *mut _, &registry as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_ecs", ud).unwrap();

            let has_health: bool = lua.load(&format!(
                "return _ecs:has({}, 'Health')", e.to_u64()
            )).eval().unwrap();
            assert!(has_health);

            let has_name: bool = lua.load(&format!(
                "return _ecs:has({}, 'Name')", e.to_u64()
            )).eval().unwrap();
            assert!(!has_name);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_ecs_spawn_despawn() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut ecs = EcsAdapter::new();
        let registry = make_registry();

        let initial_count = ecs.entity_count();

        let proxy = unsafe { EcsProxy::new(&mut ecs as *mut _, &registry as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_ecs", ud).unwrap();

            let eid: u64 = lua.load("return _ecs:spawn()").eval().unwrap();

            lua.load(&format!("_ecs:despawn({})", eid)).exec().unwrap();

            Ok(())
        }).unwrap();

        assert_eq!(ecs.entity_count(), initial_count);
    }

    #[test]
    fn test_ecs_query() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut ecs = EcsAdapter::new();
        let registry = make_registry();

        let e1 = ecs.spawn_entity();
        let e2 = ecs.spawn_entity();
        let _e3 = ecs.spawn_entity();

        ecs.set_component(e1, Health { current: 80, max: 100 }).unwrap();
        ecs.set_component(e1, PlayerTag).unwrap();
        ecs.set_component(e2, Health { current: 50, max: 50 }).unwrap();
        // e3 has nothing

        let proxy = unsafe { EcsProxy::new(&mut ecs as *mut _, &registry as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_ecs", ud).unwrap();

            // Query entities with Health
            let result: Vec<u64> = lua.load("return _ecs:query('Health')").eval().unwrap();
            assert_eq!(result.len(), 2);

            // Query entities with Health AND PlayerTag
            let result: Vec<u64> = lua.load("return _ecs:query('Health', 'PlayerTag')").eval().unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0], e1.to_u64());

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_ecs_get_nil_for_missing() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut ecs = EcsAdapter::new();
        let registry = make_registry();

        let e = ecs.spawn_entity();

        let proxy = unsafe { EcsProxy::new(&mut ecs as *mut _, &registry as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_ecs", ud).unwrap();

            let result: mlua::Value = lua.load(&format!(
                "return _ecs:get({}, 'Health')", e.to_u64()
            )).eval().unwrap();

            assert!(matches!(result, Value::Nil));

            Ok(())
        }).unwrap();
    }
}
