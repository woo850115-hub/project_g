use std::cell::RefCell;

use ecs_adapter::EntityId;
use mlua::{UserData, UserDataMethods};
use session::{LingeringEntity, SessionManager, SessionState};

/// Proxy object that Lua scripts use to query and mutate session information.
pub struct SessionProxy {
    sessions: RefCell<*mut SessionManager>,
}

// SAFETY: SessionProxy is only used within a single tick-thread scope.
unsafe impl Send for SessionProxy {}
unsafe impl Sync for SessionProxy {}

impl SessionProxy {
    /// # Safety
    /// Caller must ensure `sessions` outlives the proxy and is only used from one thread.
    pub unsafe fn new(sessions: *mut SessionManager) -> Self {
        Self {
            sessions: RefCell::new(sessions),
        }
    }

    fn with_sessions<R>(&self, f: impl FnOnce(&SessionManager) -> R) -> R {
        let ptr = *self.sessions.borrow();
        f(unsafe { &*ptr })
    }

    fn with_sessions_mut<R>(&self, f: impl FnOnce(&mut SessionManager) -> R) -> R {
        let ptr = *self.sessions.borrow();
        f(unsafe { &mut *ptr })
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

        // sessions:get_state(session_id) -> "login" | "playing" | "disconnected" | nil
        methods.add_method("get_state", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).map(|s| match s.state {
                    SessionState::Login => "login",
                    SessionState::Playing => "playing",
                    SessionState::Disconnected => "disconnected",
                })
            });
            Ok(result)
        });

        // sessions:get_account_id(session_id) -> number | nil
        methods.add_method("get_account_id", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).and_then(|s| s.account_id)
            });
            Ok(result)
        });

        // sessions:get_character_id(session_id) -> number | nil
        methods.add_method("get_character_id", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).and_then(|s| s.character_id)
            });
            Ok(result)
        });

        // sessions:get_name(session_id) -> string | nil
        methods.add_method("get_name", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).and_then(|s| s.player_name.clone())
            });
            Ok(result)
        });

        // sessions:get_entity(session_id) -> entity_id | nil
        methods.add_method("get_entity", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).and_then(|s| s.entity.map(|e| e.to_u64()))
            });
            Ok(result)
        });

        // sessions:get_permission(session_id) -> number (0=Player,1=Builder,2=Admin,3=Owner)
        methods.add_method("get_permission", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions(|sessions| {
                sessions.get_session(sid).map(|s| s.permission.as_i32()).unwrap_or(0)
            });
            Ok(result)
        });

        // sessions:start_playing(session_id, entity_id)
        // Binds entity to session and transitions to Playing state.
        methods.add_method("start_playing", |_lua, this, (sid_u64, eid_u64): (u64, u64)| {
            let sid = session::SessionId(sid_u64);
            let eid = EntityId::from_u64(eid_u64);
            this.with_sessions_mut(|sessions| {
                sessions.bind_entity(sid, eid);
            });
            Ok(())
        });

        // sessions:set_account_id(session_id, account_id)
        methods.add_method("set_account_id", |_lua, this, (sid_u64, account_id): (u64, i64)| {
            let sid = session::SessionId(sid_u64);
            this.with_sessions_mut(|sessions| {
                if let Some(s) = sessions.get_session_mut(sid) {
                    s.account_id = Some(account_id);
                }
            });
            Ok(())
        });

        // sessions:set_character_id(session_id, character_id)
        methods.add_method("set_character_id", |_lua, this, (sid_u64, character_id): (u64, i64)| {
            let sid = session::SessionId(sid_u64);
            this.with_sessions_mut(|sessions| {
                if let Some(s) = sessions.get_session_mut(sid) {
                    s.character_id = Some(character_id);
                }
            });
            Ok(())
        });

        // sessions:set_name(session_id, name)
        methods.add_method("set_name", |_lua, this, (sid_u64, name): (u64, String)| {
            let sid = session::SessionId(sid_u64);
            this.with_sessions_mut(|sessions| {
                if let Some(s) = sessions.get_session_mut(sid) {
                    s.player_name = Some(name);
                }
            });
            Ok(())
        });

        // sessions:set_permission(session_id, level)
        methods.add_method("set_permission", |_lua, this, (sid_u64, level): (u64, i32)| {
            let sid = session::SessionId(sid_u64);
            this.with_sessions_mut(|sessions| {
                if let Some(s) = sessions.get_session_mut(sid) {
                    s.permission = session::PermissionLevel::from_i32(level);
                }
            });
            Ok(())
        });

        // sessions:find_lingering(character_id) -> {entity, character_id, account_id} | nil
        methods.add_method("find_lingering", |lua, this, character_id: i64| {
            let result = this.with_sessions(|sessions| {
                sessions.find_lingering(character_id).map(|l| {
                    (l.entity.to_u64(), l.character_id, l.account_id)
                })
            });
            match result {
                Some((entity, char_id, acc_id)) => {
                    let t = lua.create_table()?;
                    t.set("entity", entity)?;
                    t.set("character_id", char_id)?;
                    t.set("account_id", acc_id)?;
                    Ok(Some(mlua::Value::Table(t)))
                }
                None => Ok(None),
            }
        });

        // sessions:rebind_lingering(session_id, character_id) -> entity_id | nil
        methods.add_method("rebind_lingering", |_lua, this, (sid_u64, character_id): (u64, i64)| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions_mut(|sessions| {
                sessions.rebind_lingering(sid, character_id).map(|e| e.to_u64())
            });
            Ok(result)
        });

        // sessions:add_lingering(entity_id, character_id, account_id, disconnect_tick)
        methods.add_method(
            "add_lingering",
            |_lua, this, (eid_u64, character_id, account_id, disconnect_tick): (u64, i64, i64, u64)| {
                let eid = EntityId::from_u64(eid_u64);
                this.with_sessions_mut(|sessions| {
                    sessions.add_lingering(LingeringEntity {
                        entity: eid,
                        character_id,
                        account_id,
                        disconnect_tick,
                    });
                });
                Ok(())
            },
        );

        // sessions:disconnect(session_id) -> entity_id | nil
        methods.add_method("disconnect", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            let result = this.with_sessions_mut(|sessions| {
                sessions.disconnect(sid).map(|e| e.to_u64())
            });
            Ok(result)
        });

        // sessions:remove_session(session_id)
        methods.add_method("remove_session", |_lua, this, sid_u64: u64| {
            let sid = session::SessionId(sid_u64);
            this.with_sessions_mut(|sessions| {
                sessions.remove_session(sid);
            });
            Ok(())
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

        let proxy = unsafe { SessionProxy::new(&mut sessions as *mut _) };
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

        let proxy = unsafe { SessionProxy::new(&mut sessions as *mut _) };
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
