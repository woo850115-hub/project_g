use std::collections::HashMap;

use plugin_abi::WasmCommand;
use wasmtime::{Caller, Linker};

/// Host-side state stored in each plugin's wasmtime::Store.
/// Accessible from host functions via Caller<'_, HostState>.
pub struct HostState {
    /// Current tick number.
    pub current_tick: u64,
    /// Deterministic random seed (tick + plugin_id based).
    pub random_seed: u64,
    /// Commands emitted by the plugin during this tick.
    pub pending_commands: Vec<WasmCommand>,
    /// Cached component data for host_get_component.
    /// Key: (entity_id_u64, component_id_u32) → serialized bytes.
    pub component_data_cache: HashMap<(u64, u32), Vec<u8>>,
}

impl HostState {
    pub fn new() -> Self {
        Self {
            current_tick: 0,
            random_seed: 0,
            pending_commands: Vec::new(),
            component_data_cache: HashMap::new(),
        }
    }
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

/// Register all host API functions on the wasmtime Linker.
pub fn register_host_functions(linker: &mut Linker<HostState>) -> Result<(), wasmtime::Error> {
    // host_emit_command(cmd_ptr: u32, cmd_len: u32) -> i32
    linker.func_wrap(
        "env",
        "host_emit_command",
        |mut caller: Caller<'_, HostState>, cmd_ptr: u32, cmd_len: u32| -> i32 {
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(mem)) => mem,
                _ => return plugin_abi::RESULT_ERR_OUT_OF_BOUNDS,
            };

            let data = memory.data(&caller);
            let start = cmd_ptr as usize;
            let end = start + cmd_len as usize;
            if end > data.len() {
                return plugin_abi::RESULT_ERR_OUT_OF_BOUNDS;
            }

            let bytes = data[start..end].to_vec();

            match plugin_abi::deserialize_command(&bytes) {
                Ok(cmd) => {
                    caller.data_mut().pending_commands.push(cmd);
                    plugin_abi::RESULT_OK
                }
                Err(_) => plugin_abi::RESULT_ERR_SERIALIZE,
            }
        },
    )?;

    // host_log(level: u32, msg_ptr: u32, msg_len: u32)
    linker.func_wrap(
        "env",
        "host_log",
        |mut caller: Caller<'_, HostState>, level: u32, msg_ptr: u32, msg_len: u32| {
            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(mem)) => mem,
                _ => return,
            };

            let data = memory.data(&caller);
            let start = msg_ptr as usize;
            let end = start + msg_len as usize;
            if end > data.len() {
                return;
            }

            let msg = String::from_utf8_lossy(&data[start..end]);
            match level {
                plugin_abi::LOG_TRACE => tracing::trace!(target: "wasm_plugin", "{}", msg),
                plugin_abi::LOG_DEBUG => tracing::debug!(target: "wasm_plugin", "{}", msg),
                plugin_abi::LOG_INFO => tracing::info!(target: "wasm_plugin", "{}", msg),
                plugin_abi::LOG_WARN => tracing::warn!(target: "wasm_plugin", "{}", msg),
                plugin_abi::LOG_ERROR => tracing::error!(target: "wasm_plugin", "{}", msg),
                _ => tracing::info!(target: "wasm_plugin", "[level={}] {}", level, msg),
            }
        },
    )?;

    // host_get_tick() -> u64
    linker.func_wrap(
        "env",
        "host_get_tick",
        |caller: Caller<'_, HostState>| -> u64 {
            caller.data().current_tick
        },
    )?;

    // host_random_seed() -> u64
    linker.func_wrap(
        "env",
        "host_random_seed",
        |caller: Caller<'_, HostState>| -> u64 {
            caller.data().random_seed
        },
    )?;

    // host_get_component(entity_id: u64, component_id: u32, out_ptr: u32, out_cap: u32) -> i32
    linker.func_wrap(
        "env",
        "host_get_component",
        |mut caller: Caller<'_, HostState>,
         entity_id: u64,
         component_id: u32,
         out_ptr: u32,
         out_cap: u32|
         -> i32 {
            // Look up cached component data
            let data_bytes = match caller.data().component_data_cache.get(&(entity_id, component_id)) {
                Some(bytes) => bytes.clone(),
                None => return plugin_abi::RESULT_ERR_ENTITY_NOT_FOUND,
            };

            let len = data_bytes.len();
            if len > out_cap as usize {
                return plugin_abi::RESULT_ERR_OUT_OF_BOUNDS;
            }

            let memory = match caller.get_export("memory") {
                Some(wasmtime::Extern::Memory(mem)) => mem,
                _ => return plugin_abi::RESULT_ERR_OUT_OF_BOUNDS,
            };

            let mem_data = memory.data_mut(&mut caller);
            let start = out_ptr as usize;
            let end = start + len;
            if end > mem_data.len() {
                return plugin_abi::RESULT_ERR_OUT_OF_BOUNDS;
            }

            mem_data[start..end].copy_from_slice(&data_bytes);
            len as i32
        },
    )?;

    Ok(())
}

/// Generate a deterministic seed from tick and plugin ID.
/// Same tick + same plugin = same seed (for deterministic PRNG in plugins).
pub fn deterministic_seed(tick: u64, plugin_id: &str) -> u64 {
    let mut hash: u64 = tick;
    for byte in plugin_id.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}
