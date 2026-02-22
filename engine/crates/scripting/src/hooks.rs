use mlua::{Function, Lua, RegistryKey, Result as LuaResult};
use std::collections::HashMap;
use tracing::warn;

/// An admin hook entry: callback + minimum required permission level.
pub struct AdminHookEntry {
    pub callback: RegistryKey,
    pub min_permission: i32,
}

/// Registry of Lua callbacks organized by event type.
pub struct HookRegistry {
    /// on_init callbacks — called once at startup
    pub on_init: Vec<RegistryKey>,
    /// on_tick callbacks — called every tick with (tick_number)
    pub on_tick: Vec<RegistryKey>,
    /// on_action callbacks — keyed by action name, called with (ctx table)
    pub on_action: HashMap<String, Vec<RegistryKey>>,
    /// on_enter_room callbacks — called with (entity_id, room_id, old_room_id)
    pub on_enter_room: Vec<RegistryKey>,
    /// on_connect callbacks — called with (session_id)
    pub on_connect: Vec<RegistryKey>,
    /// on_admin callbacks — keyed by command name, with min permission
    pub on_admin: HashMap<String, Vec<AdminHookEntry>>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self {
            on_init: Vec::new(),
            on_tick: Vec::new(),
            on_action: HashMap::new(),
            on_enter_room: Vec::new(),
            on_connect: Vec::new(),
            on_admin: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.on_init.clear();
        self.on_tick.clear();
        self.on_action.clear();
        self.on_enter_room.clear();
        self.on_connect.clear();
        self.on_admin.clear();
    }

    pub fn on_init_count(&self) -> usize {
        self.on_init.len()
    }

    pub fn on_tick_count(&self) -> usize {
        self.on_tick.len()
    }

    pub fn on_action_count(&self) -> usize {
        self.on_action.values().map(|v| v.len()).sum()
    }

    pub fn on_enter_room_count(&self) -> usize {
        self.on_enter_room.len()
    }

    pub fn on_connect_count(&self) -> usize {
        self.on_connect.len()
    }

    pub fn on_admin_count(&self) -> usize {
        self.on_admin.values().map(|v| v.len()).sum()
    }
}

/// Register hooks.* API functions on the Lua global table.
/// The HookRegistry is stored in Lua app data for callback access.
pub fn register_hooks_api(lua: &Lua) -> LuaResult<()> {
    let hooks_table = lua.create_table()?;

    // hooks.on_init(fn)
    let on_init_fn = lua.create_function(|lua, func: Function| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_init
            .push(key);
        Ok(())
    })?;
    hooks_table.set("on_init", on_init_fn)?;

    // hooks.on_tick(fn)
    let on_tick_fn = lua.create_function(|lua, func: Function| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_tick
            .push(key);
        Ok(())
    })?;
    hooks_table.set("on_tick", on_tick_fn)?;

    // hooks.on_action(action_name, fn)
    let on_action_fn = lua.create_function(|lua, (action, func): (String, Function)| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_action
            .entry(action)
            .or_default()
            .push(key);
        Ok(())
    })?;
    hooks_table.set("on_action", on_action_fn)?;

    // hooks.on_enter_room(fn)
    let on_enter_room_fn = lua.create_function(|lua, func: Function| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_enter_room
            .push(key);
        Ok(())
    })?;
    hooks_table.set("on_enter_room", on_enter_room_fn)?;

    // hooks.on_connect(fn)
    let on_connect_fn = lua.create_function(|lua, func: Function| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_connect
            .push(key);
        Ok(())
    })?;
    hooks_table.set("on_connect", on_connect_fn)?;

    // hooks.on_admin(command_name, min_permission, fn)
    let on_admin_fn = lua.create_function(|lua, (command, min_perm, func): (String, i32, Function)| {
        let key = lua.create_registry_value(func)?;
        lua.app_data_mut::<HookRegistry>()
            .expect("HookRegistry not set")
            .on_admin
            .entry(command)
            .or_default()
            .push(AdminHookEntry {
                callback: key,
                min_permission: min_perm,
            });
        Ok(())
    })?;
    hooks_table.set("on_admin", on_admin_fn)?;

    // hooks.fire_enter_room(entity_id, room_id, old_room_id_or_nil)
    // Allows Lua scripts to trigger on_enter_room hooks (e.g., after movement).
    let fire_enter_room_fn =
        lua.create_function(|lua, (entity_u64, room_u64, old_room_u64): (u64, u64, Option<u64>)| {
            // Collect functions first, then drop the borrow before calling them.
            let funcs: Vec<Function> = {
                let hooks = lua
                    .app_data_ref::<HookRegistry>()
                    .expect("HookRegistry not set");
                hooks
                    .on_enter_room
                    .iter()
                    .filter_map(|key| lua.registry_value(key).ok())
                    .collect()
            };
            for func in funcs {
                if let Err(e) = func.call::<()>((entity_u64, room_u64, old_room_u64)) {
                    warn!("on_enter_room hook error: {}", e);
                }
            }
            Ok(())
        })?;
    hooks_table.set("fire_enter_room", fire_enter_room_fn)?;

    lua.globals().set("hooks", hooks_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_registry_new() {
        let registry = HookRegistry::new();
        assert_eq!(registry.on_init_count(), 0);
        assert_eq!(registry.on_tick_count(), 0);
        assert_eq!(registry.on_action_count(), 0);
        assert_eq!(registry.on_enter_room_count(), 0);
        assert_eq!(registry.on_connect_count(), 0);
        assert_eq!(registry.on_admin_count(), 0);
    }
}
