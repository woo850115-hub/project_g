use std::cell::RefCell;

use mlua::{LuaSerdeExt, UserData, UserDataMethods};

use crate::auth::AuthProvider;

/// Proxy object that Lua scripts use to perform authentication operations.
/// Only available during on_input/on_disconnect hooks when auth is enabled.
pub struct AuthProxy {
    provider: RefCell<*const dyn AuthProvider>,
}

// SAFETY: AuthProxy is only used within a single tick-thread scope.
unsafe impl Send for AuthProxy {}
unsafe impl Sync for AuthProxy {}

impl AuthProxy {
    /// # Safety
    /// Caller must ensure `provider` outlives the proxy and is only used from one thread.
    pub unsafe fn new(provider: *const dyn AuthProvider) -> Self {
        Self {
            provider: RefCell::new(provider),
        }
    }

    fn with_provider<R>(&self, f: impl FnOnce(&dyn AuthProvider) -> R) -> R {
        let ptr = *self.provider.borrow();
        f(unsafe { &*ptr })
    }
}

impl UserData for AuthProxy {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // auth:check_account(username) -> {id, username, permission} | nil
        methods.add_method("check_account", |lua, this, username: String| {
            let result = this.with_provider(|p| p.check_account(&username));
            match result {
                Ok(Some(info)) => {
                    let t = lua.create_table()?;
                    t.set("id", info.id)?;
                    t.set("username", info.username)?;
                    t.set("permission", info.permission)?;
                    Ok(mlua::Value::Table(t))
                }
                Ok(None) => Ok(mlua::Value::Nil),
                Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
            }
        });

        // auth:authenticate(username, password) -> {id, username, permission}
        methods.add_method(
            "authenticate",
            |lua, this, (username, password): (String, String)| {
                let result = this.with_provider(|p| p.authenticate(&username, &password));
                match result {
                    Ok(info) => {
                        let t = lua.create_table()?;
                        t.set("id", info.id)?;
                        t.set("username", info.username)?;
                        t.set("permission", info.permission)?;
                        Ok(mlua::Value::Table(t))
                    }
                    Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
                }
            },
        );

        // auth:create_account(username, password) -> {id, username, permission}
        methods.add_method(
            "create_account",
            |lua, this, (username, password): (String, String)| {
                let result = this.with_provider(|p| p.create_account(&username, &password));
                match result {
                    Ok(info) => {
                        let t = lua.create_table()?;
                        t.set("id", info.id)?;
                        t.set("username", info.username)?;
                        t.set("permission", info.permission)?;
                        Ok(mlua::Value::Table(t))
                    }
                    Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
                }
            },
        );

        // auth:list_characters(account_id) -> [{id, name}, ...]
        methods.add_method("list_characters", |lua, this, account_id: i64| {
            let result = this.with_provider(|p| p.list_characters(account_id));
            match result {
                Ok(chars) => {
                    let t = lua.create_table()?;
                    for (i, c) in chars.into_iter().enumerate() {
                        let entry = lua.create_table()?;
                        entry.set("id", c.id)?;
                        entry.set("name", c.name)?;
                        t.set(i + 1, entry)?;
                    }
                    Ok(mlua::Value::Table(t))
                }
                Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
            }
        });

        // auth:create_character(account_id, name, defaults_table) -> character detail table
        methods.add_method(
            "create_character",
            |lua, this, (account_id, name, defaults): (i64, String, mlua::Value)| {
                let defaults_json: serde_json::Value = lua.from_value(defaults)?;
                let result =
                    this.with_provider(|p| p.create_character(account_id, &name, &defaults_json));
                match result {
                    Ok(detail) => {
                        let t = lua.create_table()?;
                        t.set("id", detail.id)?;
                        t.set("account_id", detail.account_id)?;
                        t.set("name", detail.name)?;
                        let comp_val: mlua::Value = lua.to_value(&detail.components)?;
                        t.set("components", comp_val)?;
                        if let Some(rid) = detail.room_id {
                            t.set("room_id", rid)?;
                        }
                        Ok(mlua::Value::Table(t))
                    }
                    Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
                }
            },
        );

        // auth:load_character(character_id) -> character detail table
        methods.add_method("load_character", |lua, this, character_id: i64| {
            let result = this.with_provider(|p| p.load_character(character_id));
            match result {
                Ok(detail) => {
                    let t = lua.create_table()?;
                    t.set("id", detail.id)?;
                    t.set("account_id", detail.account_id)?;
                    t.set("name", detail.name)?;
                    let comp_val: mlua::Value = lua.to_value(&detail.components)?;
                    t.set("components", comp_val)?;
                    if let Some(rid) = detail.room_id {
                        t.set("room_id", rid)?;
                    }
                    Ok(mlua::Value::Table(t))
                }
                Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
            }
        });

        // auth:save_character(character_id, components_table, room_id_or_nil)
        methods.add_method(
            "save_character",
            |lua, this, (character_id, components, room_id): (i64, mlua::Value, Option<u64>)| {
                let comp_json: serde_json::Value = lua.from_value(components)?;
                let result =
                    this.with_provider(|p| p.save_character(character_id, &comp_json, room_id, None));
                match result {
                    Ok(()) => Ok(()),
                    Err(e) => Err(mlua::Error::runtime(format!("{}", e))),
                }
            },
        );
    }
}
