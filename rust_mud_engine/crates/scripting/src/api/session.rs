use std::cell::RefCell;

use mlua::{UserData, UserDataMethods};
use session::SessionManager;

/// Proxy object that Lua scripts use to query session information.
pub struct SessionProxy {
    sessions: RefCell<*const SessionManager>,
}

// SAFETY: SessionProxy is only used within a single tick-thread scope.
unsafe impl Send for SessionProxy {}
unsafe impl Sync for SessionProxy {}

impl SessionProxy {
    /// # Safety
    /// Caller must ensure `sessions` outlives the proxy and is only used from one thread.
    pub unsafe fn new(sessions: *const SessionManager) -> Self {
        Self {
            sessions: RefCell::new(sessions),
        }
    }

    fn with_sessions<R>(&self, f: impl FnOnce(&SessionManager) -> R) -> R {
        let ptr = *self.sessions.borrow();
        f(unsafe { &*ptr })
    }
}

impl UserData for SessionProxy {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // sessions:session_for(entity_id) -> session_id or nil
        methods.add_method("session_for", |_lua, this, eid_u64: u64| {
            let eid = ecs_adapter::EntityId::from_u64(eid_u64);
            let result = this.with_sessions(|sessions| sessions.session_id_for_entity(eid));
            match result {
                Some(sid) => Ok(Some(sid.0)),
                None => Ok(None),
            }
        });

        // sessions:playing_list() -> [{session_id, entity, name}, ...]
        methods.add_method("playing_list", |lua, this, ()| {
            let list = this.with_sessions(|sessions| {
                sessions
                    .playing_sessions()
                    .into_iter()
                    .map(|s| {
                        (
                            s.session_id.0,
                            s.entity.map(|e| e.to_u64()),
                            s.player_name.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            });

            let result = lua.create_table()?;
            for (i, (sid, entity, name)) in list.into_iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("session_id", sid)?;
                if let Some(eid) = entity {
                    entry.set("entity", eid)?;
                }
                if let Some(n) = name {
                    entry.set("name", n)?;
                }
                result.set(i + 1, entry)?;
            }
            Ok(result)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{ScriptConfig, create_sandboxed_lua};

    #[test]
    fn test_session_for_entity() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut sessions = SessionManager::new();
        let sid = sessions.create_session();
        let eid = ecs_adapter::EntityId::new(1, 0);
        sessions.bind_entity(sid, eid);

        let proxy = unsafe { SessionProxy::new(&sessions as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_sessions", ud).unwrap();

            let result: u64 = lua
                .load(&format!(
                    "return _sessions:session_for({})",
                    eid.to_u64()
                ))
                .eval()
                .unwrap();
            assert_eq!(result, sid.0);

            // Non-existent entity returns nil
            let result: mlua::Value = lua
                .load("return _sessions:session_for(9999)")
                .eval()
                .unwrap();
            assert!(matches!(result, mlua::Value::Nil));

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_playing_list() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut sessions = SessionManager::new();
        let sid = sessions.create_session();
        let eid = ecs_adapter::EntityId::new(1, 0);
        sessions.bind_entity(sid, eid);
        if let Some(s) = sessions.get_session_mut(sid) {
            s.player_name = Some("Alice".to_string());
        }

        let proxy = unsafe { SessionProxy::new(&sessions as *const _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_sessions", ud).unwrap();

            let count: usize = lua
                .load("return #_sessions:playing_list()")
                .eval()
                .unwrap();
            assert_eq!(count, 1);

            let name: String = lua
                .load("return _sessions:playing_list()[1].name")
                .eval()
                .unwrap();
            assert_eq!(name, "Alice");

            Ok(())
        })
        .unwrap();
    }
}
