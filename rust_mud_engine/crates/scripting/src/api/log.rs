use mlua::{Lua, Result as LuaResult};

/// Register log.* API functions on the Lua global table.
/// Maps to Rust tracing macros.
pub fn register_log_api(lua: &Lua) -> LuaResult<()> {
    let log_table = lua.create_table()?;

    let info_fn = lua.create_function(|_lua, msg: String| {
        tracing::info!(target: "lua_script", "{}", msg);
        Ok(())
    })?;
    log_table.set("info", info_fn)?;

    let warn_fn = lua.create_function(|_lua, msg: String| {
        tracing::warn!(target: "lua_script", "{}", msg);
        Ok(())
    })?;
    log_table.set("warn", warn_fn)?;

    let error_fn = lua.create_function(|_lua, msg: String| {
        tracing::error!(target: "lua_script", "{}", msg);
        Ok(())
    })?;
    log_table.set("error", error_fn)?;

    let debug_fn = lua.create_function(|_lua, msg: String| {
        tracing::debug!(target: "lua_script", "{}", msg);
        Ok(())
    })?;
    log_table.set("debug", debug_fn)?;

    lua.globals().set("log", log_table)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{ScriptConfig, create_sandboxed_lua};

    #[test]
    fn test_log_api() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        register_log_api(&lua).unwrap();

        // These should not panic
        lua.load(r#"log.info("test info message")"#).exec().unwrap();
        lua.load(r#"log.warn("test warn message")"#).exec().unwrap();
        lua.load(r#"log.error("test error message")"#).exec().unwrap();
        lua.load(r#"log.debug("test debug message")"#).exec().unwrap();
    }
}
