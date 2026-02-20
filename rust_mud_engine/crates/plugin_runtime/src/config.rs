use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Global fuel configuration for the plugin runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuelConfig {
    /// Default fuel limit per plugin per tick.
    pub default_fuel_limit: u64,
    /// Max consecutive failures before quarantine.
    pub max_consecutive_failures: u32,
}

impl Default for FuelConfig {
    fn default() -> Self {
        Self {
            default_fuel_limit: 1_000_000,
            max_consecutive_failures: 3,
        }
    }
}

/// Configuration for a single plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Path to the .wasm binary.
    pub wasm_path: PathBuf,
    /// Execution priority (lower = earlier). Determines deterministic order.
    pub priority: u32,
    /// Fuel limit override (None = use FuelConfig default).
    pub fuel_limit: Option<u64>,
    /// Whether the plugin is enabled.
    pub enabled: bool,
}

/// Collection of plugin configs, sorted by priority.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugins: Vec<PluginConfig>,
}

impl PluginManifest {
    /// Return plugins sorted by priority (deterministic execution order).
    pub fn sorted(&self) -> Vec<&PluginConfig> {
        let mut sorted: Vec<&PluginConfig> = self.plugins.iter().collect();
        sorted.sort_by_key(|p| p.priority);
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_sorted_by_priority() {
        let manifest = PluginManifest {
            plugins: vec![
                PluginConfig {
                    plugin_id: "b".into(),
                    wasm_path: "b.wasm".into(),
                    priority: 10,
                    fuel_limit: None,
                    enabled: true,
                },
                PluginConfig {
                    plugin_id: "a".into(),
                    wasm_path: "a.wasm".into(),
                    priority: 1,
                    fuel_limit: None,
                    enabled: true,
                },
            ],
        };
        let sorted = manifest.sorted();
        assert_eq!(sorted[0].plugin_id, "a");
        assert_eq!(sorted[1].plugin_id, "b");
    }
}
