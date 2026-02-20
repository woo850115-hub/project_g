use std::cell::RefCell;

use mlua::{UserData, UserDataMethods, Value};
use session::{SessionId, SessionOutput};

/// Proxy for collecting session outputs from Lua scripts.
/// Outputs are accumulated and returned after script execution.
pub struct OutputProxy {
    outputs: RefCell<*mut Vec<SessionOutput>>,
}

// SAFETY: OutputProxy is only used within a single tick-thread scope.
unsafe impl Send for OutputProxy {}
unsafe impl Sync for OutputProxy {}

impl OutputProxy {
    /// # Safety
    /// Caller must ensure `outputs` outlives the proxy and is only used from one thread.
    pub unsafe fn new(outputs: *mut Vec<SessionOutput>) -> Self {
        Self {
            outputs: RefCell::new(outputs),
        }
    }

    fn push_output(&self, output: SessionOutput) {
        let ptr = *self.outputs.borrow();
        unsafe { (*ptr).push(output) };
    }
}

impl UserData for OutputProxy {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // output:send(session_id, text)
        methods.add_method("send", |_lua, this, (sid_u64, text): (u64, String)| {
            let sid = SessionId(sid_u64);
            this.push_output(SessionOutput::new(sid, text));
            Ok(())
        });

        // output:broadcast_room(room_id, text, {exclude=entity_id})
        // This collects a broadcast request. The actual expansion to
        // per-session outputs is done by the caller after script execution,
        // since we need access to space and session data.
        // For now, we store a special marker output.
        methods.add_method(
            "broadcast_room",
            |_lua, this, (room_u64, text, opts): (u64, String, Option<mlua::Table>)| {
                // Store broadcast as a special output with room info in metadata.
                // The engine will expand this to individual session outputs.
                let exclude = if let Some(ref t) = opts {
                    let val: Value = t.get("exclude")?;
                    match val {
                        Value::Integer(i) => Some(i as u64),
                        Value::Number(n) => Some(n as u64),
                        _ => None,
                    }
                } else {
                    None
                };

                // Pack broadcast info as a session output with session_id = 0 (sentinel)
                // and a formatted text that includes room/exclude metadata.
                // Format: "BROADCAST:room_u64:exclude_u64:text"
                let meta = format!(
                    "BROADCAST:{}:{}:{}",
                    room_u64,
                    exclude.unwrap_or(0),
                    text
                );
                this.push_output(SessionOutput::new(SessionId(0), meta));
                Ok(())
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::{ScriptConfig, create_sandboxed_lua};

    #[test]
    fn test_output_send() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut outputs: Vec<SessionOutput> = Vec::new();

        let proxy = unsafe { OutputProxy::new(&mut outputs as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_output", ud).unwrap();

            lua.load("_output:send(42, 'Hello, world!')").exec().unwrap();
            lua.load("_output:send(99, 'Goodbye!')").exec().unwrap();

            Ok(())
        }).unwrap();

        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].session_id, SessionId(42));
        assert_eq!(outputs[0].text, "Hello, world!");
        assert_eq!(outputs[1].session_id, SessionId(99));
        assert_eq!(outputs[1].text, "Goodbye!");
    }

    #[test]
    fn test_output_broadcast_room() {
        let lua = create_sandboxed_lua(&ScriptConfig::default()).unwrap();
        let mut outputs: Vec<SessionOutput> = Vec::new();

        let proxy = unsafe { OutputProxy::new(&mut outputs as *mut _) };
        lua.scope(|scope| {
            let ud = scope.create_userdata(proxy).unwrap();
            lua.globals().set("_output", ud).unwrap();

            lua.load("_output:broadcast_room(100, 'A loud noise echoes.', {exclude=5})")
                .exec()
                .unwrap();

            Ok(())
        }).unwrap();

        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].session_id, SessionId(0)); // sentinel
        assert!(outputs[0].text.starts_with("BROADCAST:100:5:"));
        assert!(outputs[0].text.contains("A loud noise echoes."));
    }
}
