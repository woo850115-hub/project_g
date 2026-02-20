use crate::error::ScriptError;
use mlua::Lua;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Configuration for the Lua sandbox.
#[derive(Debug, Clone)]
pub struct ScriptConfig {
    /// Memory limit in bytes (default 16 MB).
    pub memory_limit: usize,
    /// Instruction limit per execution (default 1_000_000).
    pub instruction_limit: u32,
}

impl Default for ScriptConfig {
    fn default() -> Self {
        Self {
            memory_limit: 16 * 1024 * 1024, // 16 MB
            instruction_limit: 1_000_000,
        }
    }
}

/// Create a sandboxed Luau VM with memory and instruction limits.
pub fn create_sandboxed_lua(config: &ScriptConfig) -> Result<Lua, ScriptError> {
    let lua = Lua::new();

    // Enable Luau sandbox mode — restricts access to dangerous globals
    lua.sandbox(true)?;

    // Set memory limit
    lua.set_memory_limit(config.memory_limit)?;

    // Set instruction limit via interrupt callback
    let limit = config.instruction_limit;
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    lua.set_interrupt(move |_| {
        let count = counter_clone.fetch_add(1, Ordering::Relaxed);
        if count >= limit {
            return Ok(mlua::VmState::Yield);
        }
        Ok(mlua::VmState::Continue)
    });

    Ok(lua)
}

/// Reset the instruction counter for a new execution pass.
/// Called before each hook execution batch.
pub fn reset_instruction_counter(lua: &Lua, config: &ScriptConfig) {
    let limit = config.instruction_limit;
    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();
    lua.set_interrupt(move |_| {
        let count = counter_clone.fetch_add(1, Ordering::Relaxed);
        if count >= limit {
            return Ok(mlua::VmState::Yield);
        }
        Ok(mlua::VmState::Continue)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sandboxed_lua() {
        let config = ScriptConfig::default();
        let lua = create_sandboxed_lua(&config).unwrap();

        // Basic Lua execution should work
        let result: i32 = lua.load("return 1 + 2").eval().unwrap();
        assert_eq!(result, 3);
    }

    #[test]
    fn test_sandbox_blocks_dangerous_globals() {
        let config = ScriptConfig::default();
        let lua = create_sandboxed_lua(&config).unwrap();

        // In sandbox mode, os/io/loadfile should not be accessible
        let result: mlua::Value = lua.load("return os").eval().unwrap();
        // Sandboxed Luau may return nil or restricted table for os
        // The key thing is it doesn't allow os.execute etc.
        if let mlua::Value::Table(t) = result {
            let exec: mlua::Result<mlua::Function> = t.get("execute");
            // Should not have execute
            assert!(exec.is_err() || {
                let f = exec.unwrap();
                // Calling it should fail
                f.call::<()>("echo hi").is_err()
            });
        }
        // If os is nil, that's also fine (fully sandboxed)
    }

    #[test]
    fn test_memory_limit() {
        let config = ScriptConfig {
            memory_limit: 1024 * 64, // 64 KB — very small
            instruction_limit: 10_000_000,
        };
        let lua = create_sandboxed_lua(&config).unwrap();

        // Try to allocate a large string — should fail
        let result = lua.load(r#"
            local t = {}
            for i = 1, 1000000 do
                t[i] = string.rep("x", 1000)
            end
            return #t
        "#).eval::<mlua::Value>();

        assert!(result.is_err(), "Memory allocation should fail under limit");
    }

    #[test]
    fn test_custom_config() {
        let config = ScriptConfig {
            memory_limit: 8 * 1024 * 1024,
            instruction_limit: 500_000,
        };
        let lua = create_sandboxed_lua(&config).unwrap();

        let result: i32 = lua.load("return 42").eval().unwrap();
        assert_eq!(result, 42);
    }
}
