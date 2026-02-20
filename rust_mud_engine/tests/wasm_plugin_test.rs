//! Integration tests for WASM plugin loading and execution.

use std::path::PathBuf;

use plugin_runtime::config::{FuelConfig, PluginConfig};
use plugin_runtime::PluginRuntime;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures")
        .join(name)
}

fn default_fuel() -> FuelConfig {
    FuelConfig {
        default_fuel_limit: 1_000_000,
        max_consecutive_failures: 3,
    }
}

#[test]
fn load_and_run_movement_plugin() {
    let mut runtime = PluginRuntime::new(default_fuel()).unwrap();
    let config = PluginConfig {
        plugin_id: "test_movement".into(),
        wasm_path: fixture_path("test_movement.wasm"),
        priority: 1,
        fuel_limit: None,
        enabled: true,
    };
    runtime.load_plugin(&config).unwrap();
    assert_eq!(runtime.plugin_count(), 1);

    // Run several ticks — plugin emits MoveEntity every 3 ticks
    let mut total_commands = 0;
    for tick in 0..30 {
        let cmds = runtime.run_tick(tick);
        total_commands += cmds.len();
    }

    // Should have emitted commands on ticks 0, 3, 6, 9, ... (10 out of 30)
    assert!(total_commands > 0, "plugin should have emitted commands");
    assert_eq!(total_commands, 10, "expected 10 commands over 30 ticks");
}

#[test]
fn fuel_exhaustion_stops_infinite_loop() {
    let fuel_config = FuelConfig {
        default_fuel_limit: 10_000, // Very low fuel
        max_consecutive_failures: 3,
    };
    let mut runtime = PluginRuntime::new(fuel_config).unwrap();
    let config = PluginConfig {
        plugin_id: "infinite_loop".into(),
        wasm_path: fixture_path("test_infinite_loop.wasm"),
        priority: 1,
        fuel_limit: None,
        enabled: true,
    };
    runtime.load_plugin(&config).unwrap();

    // Plugin has infinite loop but fuel should stop it
    let cmds = runtime.run_tick(0);
    assert!(cmds.is_empty(), "fuel-exhausted plugin should produce no commands");

    // Engine should still be running (not hung)
    assert_eq!(runtime.active_plugin_count(), 1);
}

#[test]
fn panic_plugin_quarantined_after_3_failures() {
    let fuel_config = FuelConfig {
        default_fuel_limit: 1_000_000,
        max_consecutive_failures: 3,
    };
    let mut runtime = PluginRuntime::new(fuel_config).unwrap();
    let config = PluginConfig {
        plugin_id: "panicker".into(),
        wasm_path: fixture_path("test_panic.wasm"),
        priority: 1,
        fuel_limit: None,
        enabled: true,
    };
    runtime.load_plugin(&config).unwrap();

    // Tick 0, 1, 2: 3 consecutive panics → quarantine
    for tick in 0..3 {
        let cmds = runtime.run_tick(tick);
        assert!(cmds.is_empty());
    }

    // After 3 failures, plugin should be quarantined
    let quarantined = runtime.quarantined_plugins();
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0], "panicker");

    // Further ticks should still work (quarantined plugin is skipped)
    let cmds = runtime.run_tick(3);
    assert!(cmds.is_empty());
    assert_eq!(runtime.active_plugin_count(), 0);
}

#[test]
fn infinite_loop_quarantined_after_3_failures() {
    let fuel_config = FuelConfig {
        default_fuel_limit: 10_000,
        max_consecutive_failures: 3,
    };
    let mut runtime = PluginRuntime::new(fuel_config).unwrap();
    let config = PluginConfig {
        plugin_id: "looper".into(),
        wasm_path: fixture_path("test_infinite_loop.wasm"),
        priority: 1,
        fuel_limit: None,
        enabled: true,
    };
    runtime.load_plugin(&config).unwrap();

    for tick in 0..3 {
        runtime.run_tick(tick);
    }

    assert_eq!(runtime.quarantined_plugins().len(), 1);
}

#[test]
fn multiple_plugins_priority_order() {
    let mut runtime = PluginRuntime::new(default_fuel()).unwrap();

    // Load movement plugin with priority 10
    runtime
        .load_plugin(&PluginConfig {
            plugin_id: "mover_b".into(),
            wasm_path: fixture_path("test_movement.wasm"),
            priority: 10,
            fuel_limit: None,
            enabled: true,
        })
        .unwrap();

    // Load another instance with priority 1 (should run first)
    runtime
        .load_plugin(&PluginConfig {
            plugin_id: "mover_a".into(),
            wasm_path: fixture_path("test_movement.wasm"),
            priority: 1,
            fuel_limit: None,
            enabled: true,
        })
        .unwrap();

    assert_eq!(runtime.plugin_count(), 2);
    assert_eq!(runtime.active_plugin_count(), 2);

    // Both should produce commands at tick 0 (tick % 3 == 0)
    let cmds = runtime.run_tick(0);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn unload_plugin() {
    let mut runtime = PluginRuntime::new(default_fuel()).unwrap();
    runtime
        .load_plugin(&PluginConfig {
            plugin_id: "temp".into(),
            wasm_path: fixture_path("test_movement.wasm"),
            priority: 1,
            fuel_limit: None,
            enabled: true,
        })
        .unwrap();
    assert_eq!(runtime.plugin_count(), 1);

    runtime.unload_plugin("temp").unwrap();
    assert_eq!(runtime.plugin_count(), 0);

    assert!(runtime.unload_plugin("nonexistent").is_err());
}
