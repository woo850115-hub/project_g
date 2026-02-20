use std::path::Path;

use ecs_adapter::{EcsAdapter, EntityId};
use mlua::{AppDataRef, Function, Lua, LuaSerdeExt};
use session::{SessionId, SessionManager, SessionOutput};
use space::model::SpaceModel;
use tracing::{info, warn};

use crate::api::ecs::EcsProxy;
use crate::api::log::register_log_api;
use crate::api::output::OutputProxy;
use crate::api::session::SessionProxy;
use crate::api::space::{IntoSpaceKind, SpaceProxy};
use crate::component_registry::ScriptComponentRegistry;
use crate::content::ContentRegistry;
use crate::error::ScriptError;
use crate::hooks::{self, HookRegistry};
use crate::sandbox::{self, ScriptConfig};

/// Context passed to script execution methods.
/// Holds mutable references to the game state that Lua scripts can access.
pub struct ScriptContext<'a, S: SpaceModel> {
    pub ecs: &'a mut EcsAdapter,
    pub space: &'a mut S,
    pub sessions: &'a SessionManager,
    pub tick: u64,
}

/// Represents a player action for on_action hooks.
pub struct ActionInfo {
    pub action_name: String,
    pub args: String,
    pub session_id: SessionId,
    pub entity: EntityId,
}

/// The main script engine managing a Luau VM and hook registry.
pub struct ScriptEngine {
    lua: Lua,
    config: ScriptConfig,
    script_count: usize,
    component_registry: ScriptComponentRegistry,
}

impl ScriptEngine {
    /// Create a new ScriptEngine with the given sandbox configuration.
    pub fn new(config: ScriptConfig) -> Result<Self, ScriptError> {
        let lua = sandbox::create_sandboxed_lua(&config)?;

        // Store HookRegistry in Lua app data so callbacks can access it
        lua.set_app_data(HookRegistry::new());

        // Register hooks.* API
        hooks::register_hooks_api(&lua)?;

        // Register log.* API
        register_log_api(&lua)?;

        info!(
            "ScriptEngine initialized (memory_limit={}KB, instruction_limit={})",
            config.memory_limit / 1024,
            config.instruction_limit
        );

        Ok(Self {
            lua,
            config,
            script_count: 0,
            component_registry: ScriptComponentRegistry::new(),
        })
    }

    /// Get a mutable reference to the component registry for registration.
    pub fn component_registry_mut(&mut self) -> &mut ScriptComponentRegistry {
        &mut self.component_registry
    }

    /// Get a reference to the component registry.
    pub fn component_registry(&self) -> &ScriptComponentRegistry {
        &self.component_registry
    }

    /// Register content data as a permanent Lua global table.
    /// Called once at startup, before loading scripts.
    /// Content is read-only — no proxy needed, just plain Lua tables.
    pub fn register_content(&self, registry: &ContentRegistry) -> Result<(), ScriptError> {
        let content_table = self.lua.create_table()?;

        for (collection_name, items) in registry.collections() {
            let col_table = self.lua.create_table()?;
            for (id, value) in items {
                let lua_val: mlua::Value = self.lua.to_value(value)?;
                col_table.set(id.as_str(), lua_val)?;
            }
            content_table.set(collection_name.as_str(), col_table)?;
        }

        self.lua.globals().set("content", content_table)?;

        Ok(())
    }

    /// Load and execute a Lua script by name and source code.
    /// Scripts typically register hooks during loading.
    pub fn load_script(&mut self, name: &str, source: &str) -> Result<(), ScriptError> {
        // Reset instruction counter before loading
        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua
            .load(source)
            .set_name(name)
            .exec()
            .map_err(|e| ScriptError::Load(format!("{}: {}", name, e)))?;

        self.script_count += 1;
        info!(script = name, "Script loaded successfully");
        Ok(())
    }

    /// Load all .lua and .luau files from a directory.
    pub fn load_directory(&mut self, path: &Path) -> Result<(), ScriptError> {
        if !path.is_dir() {
            return Err(ScriptError::Load(format!(
                "not a directory: {}",
                path.display()
            )));
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let p = e.path();
                p.extension()
                    .map(|ext| ext == "lua" || ext == "luau")
                    .unwrap_or(false)
            })
            .collect();

        // Sort for deterministic load order
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let file_path = entry.path();
            let name = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let source = std::fs::read_to_string(&file_path)?;
            self.load_script(name, &source)?;
        }

        Ok(())
    }

    /// Run all on_init hooks (called once at startup).
    /// Returns collected session outputs from Lua scripts.
    pub fn run_on_init<S: SpaceModel + IntoSpaceKind>(
        &self,
        ctx: &mut ScriptContext<'_, S>,
    ) -> Result<Vec<SessionOutput>, ScriptError> {
        let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
        if hooks.on_init.is_empty() {
            return Ok(Vec::new());
        }
        drop(hooks);

        let mut outputs = Vec::new();

        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua.scope(|scope| {
            let ecs_proxy = unsafe {
                EcsProxy::new(
                    ctx.ecs as *mut EcsAdapter,
                    &self.component_registry as *const ScriptComponentRegistry,
                )
            };
            let space_proxy = unsafe { SpaceProxy::from_space(ctx.space as *mut S) };
            let output_proxy = unsafe { OutputProxy::new(&mut outputs as *mut Vec<SessionOutput>) };
            let session_proxy = unsafe { SessionProxy::new(ctx.sessions as *const SessionManager) };

            let ecs_ud = scope.create_userdata(ecs_proxy)?;
            let space_ud = scope.create_userdata(space_proxy)?;
            let output_ud = scope.create_userdata(output_proxy)?;
            let session_ud = scope.create_userdata(session_proxy)?;

            self.lua.globals().set("ecs", ecs_ud)?;
            self.lua.globals().set("space", space_ud)?;
            self.lua.globals().set("output", output_ud)?;
            self.lua.globals().set("sessions", session_ud)?;

            let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
            for key in &hooks.on_init {
                let func: Function = self.lua.registry_value(key)?;
                if let Err(e) = func.call::<()>(()) {
                    warn!("on_init hook error: {}", e);
                }
            }

            Ok(())
        })?;

        Ok(outputs)
    }

    /// Run all on_tick hooks.
    /// Returns collected session outputs from Lua scripts.
    pub fn run_on_tick<S: SpaceModel + IntoSpaceKind>(
        &self,
        ctx: &mut ScriptContext<'_, S>,
    ) -> Result<Vec<SessionOutput>, ScriptError> {
        let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
        if hooks.on_tick.is_empty() {
            return Ok(Vec::new());
        }

        let tick = ctx.tick;
        drop(hooks);

        let mut outputs = Vec::new();

        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua.scope(|scope| {
            let ecs_proxy = unsafe {
                EcsProxy::new(
                    ctx.ecs as *mut EcsAdapter,
                    &self.component_registry as *const ScriptComponentRegistry,
                )
            };
            let space_proxy = unsafe { SpaceProxy::from_space(ctx.space as *mut S) };
            let output_proxy = unsafe { OutputProxy::new(&mut outputs as *mut Vec<SessionOutput>) };
            let session_proxy = unsafe { SessionProxy::new(ctx.sessions as *const SessionManager) };

            let ecs_ud = scope.create_userdata(ecs_proxy)?;
            let space_ud = scope.create_userdata(space_proxy)?;
            let output_ud = scope.create_userdata(output_proxy)?;
            let session_ud = scope.create_userdata(session_proxy)?;

            self.lua.globals().set("ecs", ecs_ud)?;
            self.lua.globals().set("space", space_ud)?;
            self.lua.globals().set("output", output_ud)?;
            self.lua.globals().set("sessions", session_ud)?;

            let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
            for key in &hooks.on_tick {
                let func: Function = self.lua.registry_value(key)?;
                if let Err(e) = func.call::<()>(tick) {
                    warn!("on_tick hook error: {}", e);
                }
            }

            Ok(())
        })?;

        Ok(outputs)
    }

    /// Run on_action hooks for a specific action.
    /// Returns (outputs, consumed) where consumed=true means the action was handled by Lua.
    pub fn run_on_action<S: SpaceModel + IntoSpaceKind>(
        &self,
        ctx: &mut ScriptContext<'_, S>,
        action: &ActionInfo,
    ) -> Result<(Vec<SessionOutput>, bool), ScriptError> {
        let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
        let callbacks = hooks.on_action.get(&action.action_name);
        if callbacks.is_none() || callbacks.unwrap().is_empty() {
            return Ok((Vec::new(), false));
        }
        drop(hooks);

        let mut outputs = Vec::new();
        let mut consumed = false;

        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua.scope(|scope| {
            let ecs_proxy = unsafe {
                EcsProxy::new(
                    ctx.ecs as *mut EcsAdapter,
                    &self.component_registry as *const ScriptComponentRegistry,
                )
            };
            let space_proxy = unsafe { SpaceProxy::from_space(ctx.space as *mut S) };
            let output_proxy = unsafe { OutputProxy::new(&mut outputs as *mut Vec<SessionOutput>) };
            let session_proxy = unsafe { SessionProxy::new(ctx.sessions as *const SessionManager) };

            let ecs_ud = scope.create_userdata(ecs_proxy)?;
            let space_ud = scope.create_userdata(space_proxy)?;
            let output_ud = scope.create_userdata(output_proxy)?;
            let session_ud = scope.create_userdata(session_proxy)?;

            self.lua.globals().set("ecs", ecs_ud)?;
            self.lua.globals().set("space", space_ud)?;
            self.lua.globals().set("output", output_ud)?;
            self.lua.globals().set("sessions", session_ud)?;

            // Build context table for the callback
            let action_ctx = self.lua.create_table()?;
            action_ctx.set("session_id", action.session_id.0)?;
            action_ctx.set("entity", action.entity.to_u64())?;
            action_ctx.set("action", action.action_name.as_str())?;
            action_ctx.set("args", action.args.as_str())?;

            let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
            if let Some(callbacks) = hooks.on_action.get(&action.action_name) {
                for key in callbacks {
                    let func: Function = self.lua.registry_value(key)?;
                    match func.call::<mlua::Value>(action_ctx.clone()) {
                        Ok(mlua::Value::Boolean(true)) => {
                            consumed = true;
                            break;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!("on_action('{}') hook error: {}", action.action_name, e);
                        }
                    }
                }
            }

            Ok(())
        })?;

        Ok((outputs, consumed))
    }

    /// Run on_enter_room hooks.
    pub fn run_on_enter_room<S: SpaceModel + IntoSpaceKind>(
        &self,
        ctx: &mut ScriptContext<'_, S>,
        entity: EntityId,
        room: EntityId,
        old_room: Option<EntityId>,
    ) -> Result<Vec<SessionOutput>, ScriptError> {
        let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
        if hooks.on_enter_room.is_empty() {
            return Ok(Vec::new());
        }
        drop(hooks);

        let mut outputs = Vec::new();

        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua.scope(|scope| {
            let ecs_proxy = unsafe {
                EcsProxy::new(
                    ctx.ecs as *mut EcsAdapter,
                    &self.component_registry as *const ScriptComponentRegistry,
                )
            };
            let space_proxy = unsafe { SpaceProxy::from_space(ctx.space as *mut S) };
            let output_proxy = unsafe { OutputProxy::new(&mut outputs as *mut Vec<SessionOutput>) };
            let session_proxy = unsafe { SessionProxy::new(ctx.sessions as *const SessionManager) };

            let ecs_ud = scope.create_userdata(ecs_proxy)?;
            let space_ud = scope.create_userdata(space_proxy)?;
            let output_ud = scope.create_userdata(output_proxy)?;
            let session_ud = scope.create_userdata(session_proxy)?;

            self.lua.globals().set("ecs", ecs_ud)?;
            self.lua.globals().set("space", space_ud)?;
            self.lua.globals().set("output", output_ud)?;
            self.lua.globals().set("sessions", session_ud)?;

            let entity_u64 = entity.to_u64();
            let room_u64 = room.to_u64();
            let old_room_val: mlua::Value = match old_room {
                Some(r) => mlua::Value::Number(r.to_u64() as f64),
                None => mlua::Value::Nil,
            };

            let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
            for key in &hooks.on_enter_room {
                let func: Function = self.lua.registry_value(key)?;
                if let Err(e) = func.call::<()>((entity_u64, room_u64, old_room_val.clone())) {
                    warn!("on_enter_room hook error: {}", e);
                }
            }

            Ok(())
        })?;

        Ok(outputs)
    }

    /// Run on_connect hooks.
    pub fn run_on_connect<S: SpaceModel + IntoSpaceKind>(
        &self,
        ctx: &mut ScriptContext<'_, S>,
        session_id: SessionId,
    ) -> Result<Vec<SessionOutput>, ScriptError> {
        let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
        if hooks.on_connect.is_empty() {
            return Ok(Vec::new());
        }
        drop(hooks);

        let mut outputs = Vec::new();

        sandbox::reset_instruction_counter(&self.lua, &self.config);

        self.lua.scope(|scope| {
            let ecs_proxy = unsafe {
                EcsProxy::new(
                    ctx.ecs as *mut EcsAdapter,
                    &self.component_registry as *const ScriptComponentRegistry,
                )
            };
            let space_proxy = unsafe { SpaceProxy::from_space(ctx.space as *mut S) };
            let output_proxy = unsafe { OutputProxy::new(&mut outputs as *mut Vec<SessionOutput>) };
            let session_proxy = unsafe { SessionProxy::new(ctx.sessions as *const SessionManager) };

            let ecs_ud = scope.create_userdata(ecs_proxy)?;
            let space_ud = scope.create_userdata(space_proxy)?;
            let output_ud = scope.create_userdata(output_proxy)?;
            let session_ud = scope.create_userdata(session_proxy)?;

            self.lua.globals().set("ecs", ecs_ud)?;
            self.lua.globals().set("space", space_ud)?;
            self.lua.globals().set("output", output_ud)?;
            self.lua.globals().set("sessions", session_ud)?;

            let hooks = self.lua.app_data_ref::<HookRegistry>().unwrap();
            for key in &hooks.on_connect {
                let func: Function = self.lua.registry_value(key)?;
                if let Err(e) = func.call::<()>(session_id.0) {
                    warn!("on_connect hook error: {}", e);
                }
            }

            Ok(())
        })?;

        Ok(outputs)
    }

    /// Get a reference to the underlying Lua VM.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Get the sandbox configuration.
    pub fn config(&self) -> &ScriptConfig {
        &self.config
    }

    /// Number of scripts loaded.
    pub fn script_count(&self) -> usize {
        self.script_count
    }

    /// Access the hook registry (read-only).
    pub fn hook_registry(&self) -> AppDataRef<'_, HookRegistry> {
        self.lua
            .app_data_ref::<HookRegistry>()
            .expect("HookRegistry not in app_data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_registry::ScriptComponent;
    use ecs_adapter::Component;
    use mlua::LuaSerdeExt;
    use serde::{Deserialize, Serialize};
    use space::RoomGraphSpace;
    use space::room_graph::RoomExits;

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct Health {
        current: i32,
        max: i32,
    }

    struct HealthHandler;
    impl ScriptComponent for HealthHandler {
        fn tag(&self) -> &str {
            "Health"
        }
        fn get_as_lua(
            &self,
            ecs: &EcsAdapter,
            eid: EntityId,
            lua: &Lua,
        ) -> Result<Option<mlua::Value>, ScriptError> {
            match ecs.get_component::<Health>(eid) {
                Ok(c) => {
                    let json_val = serde_json::to_value(c).unwrap();
                    Ok(Some(lua.to_value(&json_val)?))
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
            let json: serde_json::Value = lua.from_value(value)?;
            let c: Health = serde_json::from_value(json).unwrap();
            ecs.set_component(eid, c).unwrap();
            Ok(())
        }
        fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool {
            ecs.has_component::<Health>(eid)
        }
        fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError> {
            ecs.remove_component::<Health>(eid).unwrap();
            Ok(())
        }
        fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId> {
            ecs.entities_with::<Health>()
        }
    }

    fn setup_world() -> (EcsAdapter, RoomGraphSpace, SessionManager) {
        let ecs = EcsAdapter::new();
        let mut space = RoomGraphSpace::new();
        let room_a = EntityId::new(100, 0);
        let room_b = EntityId::new(101, 0);
        space.register_room(
            room_a,
            RoomExits {
                north: Some(room_b),
                ..Default::default()
            },
        );
        space.register_room(
            room_b,
            RoomExits {
                south: Some(room_a),
                ..Default::default()
            },
        );
        let sessions = SessionManager::new();
        (ecs, space, sessions)
    }

    #[test]
    fn test_engine_new() {
        let engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        assert_eq!(engine.script_count(), 0);
    }

    #[test]
    fn test_load_script_basic() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine.load_script("test", "local x = 1 + 2").unwrap();
        assert_eq!(engine.script_count(), 1);
    }

    #[test]
    fn test_load_script_registers_hooks() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine
            .load_script(
                "test_hooks",
                r#"
                hooks.on_tick(function(tick)
                    -- do something
                end)
                hooks.on_action("attack", function(ctx)
                    return true
                end)
            "#,
            )
            .unwrap();

        assert_eq!(engine.hook_registry().on_tick_count(), 1);
        assert_eq!(engine.hook_registry().on_action_count(), 1);
    }

    #[test]
    fn test_load_script_syntax_error() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        let result = engine.load_script("bad", "this is not valid lua }{}{");
        assert!(result.is_err());
    }

    #[test]
    fn test_load_directory() {
        let dir = std::env::temp_dir().join("scripting_test_load_dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("01_first.lua"), "hooks.on_tick(function() end)").unwrap();
        std::fs::write(dir.join("02_second.lua"), "hooks.on_tick(function() end)").unwrap();
        std::fs::write(dir.join("readme.txt"), "not a lua file").unwrap();

        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine.load_directory(&dir).unwrap();

        assert_eq!(engine.script_count(), 2);
        assert_eq!(engine.hook_registry().on_tick_count(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_directory_not_exists() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        let result = engine.load_directory(Path::new("/tmp/nonexistent_scripting_dir"));
        assert!(result.is_err());
    }

    #[test]
    fn test_run_on_tick() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine
            .component_registry_mut()
            .register(Box::new(HealthHandler));

        engine
            .load_script(
                "tick_test",
                r#"
                hooks.on_tick(function(tick)
                    log.info("tick " .. tostring(tick))
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 42,
        };

        let outputs = engine.run_on_tick(&mut ctx).unwrap();
        // No outputs expected (just logging)
        assert!(outputs.is_empty());
    }

    #[test]
    fn test_run_on_tick_with_output() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        engine
            .load_script(
                "tick_output",
                r#"
                hooks.on_tick(function(tick)
                    output:send(1, "Tick " .. tostring(tick))
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 5,
        };

        let outputs = engine.run_on_tick(&mut ctx).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].session_id, SessionId(1));
        assert_eq!(outputs[0].text, "Tick 5");
    }

    #[test]
    fn test_run_on_action_consumed() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        engine
            .load_script(
                "action_test",
                r#"
                hooks.on_action("dance", function(ctx)
                    output:send(ctx.session_id, "You dance!")
                    return true
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let entity = ecs.spawn_entity();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        let action = ActionInfo {
            action_name: "dance".to_string(),
            args: String::new(),
            session_id: SessionId(42),
            entity,
        };

        let (outputs, consumed) = engine.run_on_action(&mut ctx, &action).unwrap();
        assert!(consumed);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "You dance!");
    }

    #[test]
    fn test_run_on_action_not_consumed() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        engine
            .load_script(
                "action_test",
                r#"
                hooks.on_action("dance", function(ctx)
                    -- do something but don't consume
                    return false
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let entity = ecs.spawn_entity();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        let action = ActionInfo {
            action_name: "dance".to_string(),
            args: String::new(),
            session_id: SessionId(42),
            entity,
        };

        let (_outputs, consumed) = engine.run_on_action(&mut ctx, &action).unwrap();
        assert!(!consumed);
    }

    #[test]
    fn test_run_on_action_no_handler() {
        let engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let entity = ecs.spawn_entity();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        let action = ActionInfo {
            action_name: "nonexistent".to_string(),
            args: String::new(),
            session_id: SessionId(1),
            entity,
        };

        let (outputs, consumed) = engine.run_on_action(&mut ctx, &action).unwrap();
        assert!(!consumed);
        assert!(outputs.is_empty());
    }

    #[test]
    fn test_run_on_enter_room() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        engine
            .load_script(
                "enter_room_test",
                r#"
                hooks.on_enter_room(function(entity, room, old_room)
                    output:send(1, "Entity entered room")
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let entity = EntityId::new(1, 0);
        let room = EntityId::new(100, 0);

        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        let outputs = engine
            .run_on_enter_room(&mut ctx, entity, room, None)
            .unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "Entity entered room");
    }

    #[test]
    fn test_run_on_connect() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();

        engine
            .load_script(
                "connect_test",
                r#"
                hooks.on_connect(function(session_id)
                    output:send(session_id, "Welcome!")
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        let outputs = engine.run_on_connect(&mut ctx, SessionId(7)).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].session_id, SessionId(7));
        assert_eq!(outputs[0].text, "Welcome!");
    }

    #[test]
    fn test_on_tick_ecs_access() {
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine
            .component_registry_mut()
            .register(Box::new(HealthHandler));

        engine
            .load_script(
                "ecs_access",
                r#"
                hooks.on_tick(function(tick)
                    local entities = ecs:query("Health")
                    for _, eid in ipairs(entities) do
                        local hp = ecs:get(eid, "Health")
                        if hp then
                            hp.current = hp.current - 1
                            ecs:set(eid, "Health", hp)
                        end
                    end
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let entity = ecs.spawn_entity();
        ecs.set_component(entity, Health { current: 10, max: 10 })
            .unwrap();

        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };

        engine.run_on_tick(&mut ctx).unwrap();

        // Health should have been decremented
        let hp = ctx.ecs.get_component::<Health>(entity).unwrap();
        assert_eq!(hp.current, 9);
        assert_eq!(hp.max, 10);
    }

    #[test]
    fn test_register_content_basic() {
        let dir = std::env::temp_dir().join("engine_content_test_basic");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("monsters.json"),
            r#"[{"id":"goblin","name":"Goblin","hp":30},{"id":"orc","name":"Orc","hp":80}]"#,
        )
        .unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine.register_content(&registry).unwrap();

        engine
            .load_script(
                "test",
                r#"
                hooks.on_init(function()
                    local g = content.monsters.goblin
                    output:send(1, g.name .. ":" .. tostring(g.hp))
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 0,
        };
        let outputs = engine.run_on_init(&mut ctx).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "Goblin:30");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_register_content_empty() {
        let registry = ContentRegistry::new();
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine.register_content(&registry).unwrap();

        engine
            .load_script(
                "test",
                r#"
                hooks.on_init(function()
                    if content.monsters == nil then
                        output:send(1, "nil")
                    else
                        output:send(1, "exists")
                    end
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 0,
        };
        let outputs = engine.run_on_init(&mut ctx).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "nil");
    }

    #[test]
    fn test_content_accessible_from_hooks() {
        let dir = std::env::temp_dir().join("engine_content_test_hooks");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("items.json"),
            r#"[{"id":"potion","name":"Health Potion","heal":50}]"#,
        )
        .unwrap();

        let registry = ContentRegistry::load_dir(&dir).unwrap();
        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine.register_content(&registry).unwrap();

        // Test from on_tick hook (not just on_init)
        engine
            .load_script(
                "test",
                r#"
                hooks.on_tick(function(tick)
                    local p = content.items.potion
                    if p then
                        output:send(1, p.name .. ":" .. tostring(p.heal))
                    end
                end)
            "#,
            )
            .unwrap();

        let (mut ecs, mut space, sessions) = setup_world();
        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };
        let outputs = engine.run_on_tick(&mut ctx).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "Health Potion:50");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_run_on_tick_with_grid_space() {
        use space::grid_space::{GridConfig, GridSpace};

        let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
        engine
            .load_script(
                "grid_tick",
                r#"
                hooks.on_tick(function(tick)
                    local count = space:entity_count()
                    output:send(1, "entities: " .. tostring(count))
                end)
            "#,
            )
            .unwrap();

        let mut ecs = EcsAdapter::new();
        let mut grid = GridSpace::new(GridConfig {
            width: 10,
            height: 10,
            origin_x: 0,
            origin_y: 0,
        });
        let sessions = SessionManager::new();

        let entity = ecs.spawn_entity();
        grid.set_position(entity, 3, 4).unwrap();

        let mut ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut grid,
            sessions: &sessions,
            tick: 1,
        };

        let outputs = engine.run_on_tick(&mut ctx).unwrap();
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].text, "entities: 1");
    }
}
