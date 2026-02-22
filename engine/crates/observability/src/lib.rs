use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
}

#[derive(Debug, Clone)]
pub struct TickMetrics {
    pub tick_number: u64,
    pub duration_us: u128,
    pub command_count: usize,
    pub entity_count: usize,
    /// WASM plugin execution time in microseconds (0 if no plugins).
    pub wasm_duration_us: u128,
}

impl TickMetrics {
    pub fn log(&self) {
        const TICK_BUDGET_US: u128 = 33_000;
        if self.duration_us > TICK_BUDGET_US {
            tracing::warn!(
                tick = self.tick_number,
                duration_us = self.duration_us,
                wasm_us = self.wasm_duration_us,
                commands = self.command_count,
                entities = self.entity_count,
                "tick exceeded budget ({}us > {}us)",
                self.duration_us,
                TICK_BUDGET_US
            );
        } else {
            tracing::info!(
                tick = self.tick_number,
                duration_us = self.duration_us,
                wasm_us = self.wasm_duration_us,
                commands = self.command_count,
                entities = self.entity_count,
                "tick completed"
            );
        }
    }
}
