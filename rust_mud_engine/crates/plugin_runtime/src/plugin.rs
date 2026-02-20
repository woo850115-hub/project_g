use wasmtime::{Engine, Instance, Module, Store, TypedFunc};

use crate::config::{FuelConfig, PluginConfig};
use crate::error::{PluginError, PluginExecResult};
use crate::host_api::{deterministic_seed, HostState};

/// Plugin lifecycle state.
#[derive(Debug, Clone)]
pub enum PluginState {
    Active,
    Quarantined {
        since_tick: u64,
        reason: String,
    },
}

/// A loaded and ready-to-execute WASM plugin.
pub struct LoadedPlugin {
    pub id: String,
    pub priority: u32,
    pub fuel_limit: u64,
    pub state: PluginState,
    pub consecutive_failures: u32,
    max_consecutive_failures: u32,
    store: Store<HostState>,
    #[allow(dead_code)]
    instance: Instance,
    fn_on_tick: TypedFunc<u64, i32>,
}

impl LoadedPlugin {
    /// Load a WASM plugin from binary bytes.
    pub fn from_bytes(
        engine: &Engine,
        wasm_bytes: &[u8],
        config: &PluginConfig,
        fuel_config: &FuelConfig,
        linker: &wasmtime::Linker<HostState>,
    ) -> Result<Self, PluginError> {
        let module = Module::new(engine, wasm_bytes)
            .map_err(|e| PluginError::LoadError(format!("failed to compile module: {}", e)))?;

        let mut store = Store::new(engine, HostState::new());
        store.set_fuel(fuel_config.default_fuel_limit).map_err(|e| {
            PluginError::LoadError(format!("failed to set initial fuel: {}", e))
        })?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| PluginError::LoadError(format!("failed to instantiate: {}", e)))?;

        let fn_on_tick = instance
            .get_typed_func::<u64, i32>(&mut store, "on_tick")
            .map_err(|e| PluginError::MissingExport(format!("on_tick: {}", e)))?;

        // Call on_load if exported
        if let Ok(on_load) = instance.get_typed_func::<(), i32>(&mut store, "on_load") {
            store.set_fuel(fuel_config.default_fuel_limit)?;
            match on_load.call(&mut store, ()) {
                Ok(0) => {}
                Ok(code) => {
                    return Err(PluginError::LoadError(format!(
                        "on_load returned error code: {}",
                        code
                    )));
                }
                Err(e) => {
                    return Err(PluginError::LoadError(format!(
                        "on_load trapped: {}",
                        e
                    )));
                }
            }
        }

        let fuel_limit = config.fuel_limit.unwrap_or(fuel_config.default_fuel_limit);

        Ok(Self {
            id: config.plugin_id.clone(),
            priority: config.priority,
            fuel_limit,
            state: PluginState::Active,
            consecutive_failures: 0,
            max_consecutive_failures: fuel_config.max_consecutive_failures,
            store,
            instance,
            fn_on_tick,
        })
    }

    /// Check if the plugin is quarantined.
    pub fn is_quarantined(&self) -> bool {
        matches!(self.state, PluginState::Quarantined { .. })
    }

    /// Execute on_tick for this plugin. Returns collected commands or failure info.
    pub fn execute_tick(&mut self, tick: u64) -> PluginExecResult {
        if self.is_quarantined() {
            return PluginExecResult::Trapped(format!("plugin {} is quarantined", self.id));
        }

        // Prepare host state for this tick
        self.store.data_mut().current_tick = tick;
        self.store.data_mut().random_seed = deterministic_seed(tick, &self.id);
        self.store.data_mut().pending_commands.clear();

        // Refill fuel
        if let Err(e) = self.store.set_fuel(self.fuel_limit) {
            return PluginExecResult::Trapped(format!("failed to set fuel: {}", e));
        }

        // Call on_tick
        match self.fn_on_tick.call(&mut self.store, tick) {
            Ok(plugin_abi::RESULT_OK) => {
                self.consecutive_failures = 0;
                let commands = std::mem::take(&mut self.store.data_mut().pending_commands);
                PluginExecResult::Success(commands)
            }
            Ok(error_code) => {
                // Plugin returned non-zero (application error, not trap)
                self.consecutive_failures = 0;
                tracing::warn!(
                    plugin = %self.id,
                    tick = tick,
                    error_code = error_code,
                    "plugin returned error code"
                );
                let commands = std::mem::take(&mut self.store.data_mut().pending_commands);
                PluginExecResult::Success(commands)
            }
            Err(trap) => {
                // Discard any partial commands (implicit rollback)
                self.store.data_mut().pending_commands.clear();
                self.consecutive_failures += 1;

                let is_fuel = trap
                    .downcast_ref::<wasmtime::Trap>()
                    .is_some_and(|t| matches!(t, wasmtime::Trap::OutOfFuel));

                if is_fuel {
                    tracing::warn!(
                        plugin = %self.id,
                        tick = tick,
                        consecutive = self.consecutive_failures,
                        "plugin fuel exhausted — commands discarded"
                    );
                    self.maybe_quarantine(tick);
                    PluginExecResult::FuelExceeded
                } else {
                    let msg = trap.to_string();
                    tracing::warn!(
                        plugin = %self.id,
                        tick = tick,
                        consecutive = self.consecutive_failures,
                        error = %msg,
                        "plugin trapped — commands discarded"
                    );
                    self.maybe_quarantine(tick);
                    PluginExecResult::Trapped(msg)
                }
            }
        }
    }

    /// Populate the component data cache from the ECS for this plugin's tick.
    pub fn populate_component_cache(
        &mut self,
        cache: std::collections::HashMap<(u64, u32), Vec<u8>>,
    ) {
        self.store.data_mut().component_data_cache = cache;
    }

    fn maybe_quarantine(&mut self, tick: u64) {
        if self.consecutive_failures >= self.max_consecutive_failures {
            let reason = format!(
                "{} consecutive failures",
                self.consecutive_failures
            );
            tracing::error!(
                plugin = %self.id,
                tick = tick,
                reason = %reason,
                "plugin quarantined"
            );
            self.state = PluginState::Quarantined {
                since_tick: tick,
                reason,
            };
        }
    }
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("state", &self.state)
            .field("consecutive_failures", &self.consecutive_failures)
            .finish()
    }
}
