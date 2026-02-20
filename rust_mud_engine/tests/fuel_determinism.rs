//! Determinism test: same seed + same fuel = same result.

use std::path::PathBuf;

use plugin_runtime::config::{FuelConfig, PluginConfig};
use plugin_runtime::PluginRuntime;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures")
        .join(name)
}

fn run_simulation(fuel_limit: u64, ticks: u64) -> Vec<Vec<plugin_abi::WasmCommand>> {
    let fuel_config = FuelConfig {
        default_fuel_limit: fuel_limit,
        max_consecutive_failures: 3,
    };
    let mut runtime = PluginRuntime::new(fuel_config).unwrap();
    runtime
        .load_plugin(&PluginConfig {
            plugin_id: "test_movement".into(),
            wasm_path: fixture_path("test_movement.wasm"),
            priority: 1,
            fuel_limit: None,
            enabled: true,
        })
        .unwrap();

    let mut all_ticks = Vec::new();
    for tick in 0..ticks {
        let cmds = runtime.run_tick(tick);
        all_ticks.push(cmds);
    }
    all_ticks
}

#[test]
fn same_fuel_same_result() {
    let run_a = run_simulation(1_000_000, 30);
    let run_b = run_simulation(1_000_000, 30);

    assert_eq!(run_a.len(), run_b.len());
    for (tick, (a, b)) in run_a.iter().zip(run_b.iter()).enumerate() {
        assert_eq!(a, b, "commands diverged at tick {}", tick);
    }
}

#[test]
fn deterministic_across_100_ticks() {
    let run_a = run_simulation(1_000_000, 100);
    let run_b = run_simulation(1_000_000, 100);

    for (tick, (a, b)) in run_a.iter().zip(run_b.iter()).enumerate() {
        assert_eq!(a, b, "commands diverged at tick {}", tick);
    }

    // Verify that some commands were actually produced
    let total: usize = run_a.iter().map(|v| v.len()).sum();
    assert!(total > 0, "expected some commands to be produced");
}
