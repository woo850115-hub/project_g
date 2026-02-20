pub mod config;
pub mod error;
pub mod host_api;
pub mod memory;
pub mod plugin;
pub mod registry;
pub mod serializer;

use plugin_abi::WasmCommand;
use wasmtime::{Engine, Linker};

use crate::config::{FuelConfig, PluginConfig};
use crate::error::{PluginError, PluginExecResult};
use crate::host_api::HostState;
use crate::plugin::LoadedPlugin;
use crate::registry::ComponentRegistry;

pub use crate::config::FuelConfig as FuelCfg;
pub use crate::error::PluginError as Error;
pub use crate::plugin::PluginState;
pub use crate::registry::ComponentRegistry as Registry;
pub use plugin_abi::WasmCommand as WasmCmd;

/// The main WASM plugin runtime.
/// Manages plugin loading, execution, and lifecycle.
pub struct PluginRuntime {
    engine: Engine,
    linker: Linker<HostState>,
    plugins: Vec<LoadedPlugin>,
    fuel_config: FuelConfig,
    pub registry: ComponentRegistry,
}

impl PluginRuntime {
    /// Create a new plugin runtime with the given fuel configuration.
    pub fn new(fuel_config: FuelConfig) -> Result<Self, PluginError> {
        let mut wasm_config = wasmtime::Config::new();
        wasm_config.consume_fuel(true);

        let engine = Engine::new(&wasm_config)?;
        let mut linker = Linker::new(&engine);
        host_api::register_host_functions(&mut linker)?;

        Ok(Self {
            engine,
            linker,
            plugins: Vec::new(),
            fuel_config,
            registry: ComponentRegistry::new(),
        })
    }

    /// Load a plugin from a .wasm file path.
    pub fn load_plugin(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if !config.enabled {
            tracing::info!(plugin = %config.plugin_id, "plugin disabled, skipping");
            return Ok(());
        }

        let wasm_bytes = std::fs::read(&config.wasm_path).map_err(|e| {
            PluginError::LoadError(format!(
                "failed to read {}: {}",
                config.wasm_path.display(),
                e
            ))
        })?;

        self.load_plugin_from_bytes(&wasm_bytes, config)
    }

    /// Load a plugin from raw WASM bytes (useful for testing).
    pub fn load_plugin_from_bytes(
        &mut self,
        wasm_bytes: &[u8],
        config: &PluginConfig,
    ) -> Result<(), PluginError> {
        let plugin = LoadedPlugin::from_bytes(
            &self.engine,
            wasm_bytes,
            config,
            &self.fuel_config,
            &self.linker,
        )?;

        tracing::info!(
            plugin = %config.plugin_id,
            priority = config.priority,
            "plugin loaded"
        );

        // Insert maintaining priority order
        let pos = self
            .plugins
            .binary_search_by_key(&plugin.priority, |p| p.priority)
            .unwrap_or_else(|pos| pos);
        self.plugins.insert(pos, plugin);

        Ok(())
    }

    /// Execute all active plugins for a tick.
    /// Returns collected WasmCommands from all plugins (in priority order).
    /// Conversion to EngineCommand is the caller's responsibility.
    pub fn run_tick(&mut self, tick: u64) -> Vec<WasmCommand> {
        let mut all_commands = Vec::new();

        for plugin in &mut self.plugins {
            if plugin.is_quarantined() {
                continue;
            }

            match plugin.execute_tick(tick) {
                PluginExecResult::Success(wasm_cmds) => {
                    all_commands.extend(wasm_cmds);
                }
                PluginExecResult::FuelExceeded | PluginExecResult::Trapped(_) => {
                    // Commands already discarded inside execute_tick
                }
            }
        }

        all_commands
    }

    /// Unload a plugin by ID.
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let pos = self
            .plugins
            .iter()
            .position(|p| p.id == plugin_id)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?;
        self.plugins.remove(pos);
        tracing::info!(plugin = %plugin_id, "plugin unloaded");
        Ok(())
    }

    /// Get IDs of quarantined plugins.
    pub fn quarantined_plugins(&self) -> Vec<&str> {
        self.plugins
            .iter()
            .filter(|p| p.is_quarantined())
            .map(|p| p.id.as_str())
            .collect()
    }

    /// Get number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Get number of active (non-quarantined) plugins.
    pub fn active_plugin_count(&self) -> usize {
        self.plugins.iter().filter(|p| !p.is_quarantined()).count()
    }
}
