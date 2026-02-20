use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("wasm trap: {0}")]
    WasmTrap(String),

    #[error("fuel exhausted for plugin {plugin_id} at tick {tick}")]
    FuelExceeded { plugin_id: String, tick: u64 },

    #[error("memory out of bounds: offset={offset}, len={len}, memory_size={memory_size}")]
    MemoryOutOfBounds {
        offset: u32,
        len: u32,
        memory_size: usize,
    },

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("plugin {0} is quarantined")]
    Quarantined(String),

    #[error("failed to load plugin: {0}")]
    LoadError(String),

    #[error("plugin {0} not found")]
    PluginNotFound(String),

    #[error("missing wasm export: {0}")]
    MissingExport(String),

    #[error("wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
}

/// Result of a single plugin execution within a tick.
#[derive(Debug)]
pub enum PluginExecResult {
    /// Plugin executed successfully, commands collected.
    Success(Vec<plugin_abi::WasmCommand>),
    /// Plugin exceeded its fuel budget.
    FuelExceeded,
    /// Plugin trapped (panic, OOB, etc).
    Trapped(String),
}

impl fmt::Display for PluginExecResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success(cmds) => write!(f, "success ({} commands)", cmds.len()),
            Self::FuelExceeded => write!(f, "fuel exceeded"),
            Self::Trapped(msg) => write!(f, "trapped: {}", msg),
        }
    }
}
