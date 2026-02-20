use std::time::{Duration, Instant};

use ecs_adapter::{ComponentId, EcsAdapter, EntityId, EventId};
use plugin_abi::WasmCommand;
use space::SpaceModel;

use crate::command::{CommandStream, EngineCommand};
use crate::events::EventBus;

/// Tick loop configuration.
#[derive(Debug, Clone)]
pub struct TickConfig {
    /// Ticks per second.
    pub tps: u32,
    /// Maximum ticks to run (0 = unlimited).
    pub max_ticks: u64,
}

impl Default for TickConfig {
    fn default() -> Self {
        Self {
            tps: 30,
            max_ticks: 0,
        }
    }
}

impl TickConfig {
    pub fn tick_duration(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.tps as f64)
    }
}

/// The main simulation tick loop combining all subsystems.
pub struct TickLoop<S: SpaceModel> {
    pub ecs: EcsAdapter,
    pub space: S,
    pub commands: CommandStream,
    pub event_bus: EventBus,
    pub config: TickConfig,
    pub current_tick: u64,
    /// Optional WASM plugin runtime. None = no plugins (Phase 0 compatible).
    pub plugin_runtime: Option<plugin_runtime::PluginRuntime>,
}

impl<S: SpaceModel> TickLoop<S> {
    pub fn new(config: TickConfig, space: S) -> Self {
        Self {
            ecs: EcsAdapter::new(),
            space,
            commands: CommandStream::new(),
            event_bus: EventBus::new(),
            config,
            current_tick: 0,
            plugin_runtime: None,
        }
    }

    /// Create a tick loop with a plugin runtime attached.
    pub fn with_plugin_runtime(config: TickConfig, space: S, runtime: plugin_runtime::PluginRuntime) -> Self {
        Self {
            plugin_runtime: Some(runtime),
            ..Self::new(config, space)
        }
    }

    /// Execute a single tick: plugins → resolve commands → apply → drain events → metrics.
    pub fn step(&mut self) -> observability::TickMetrics {
        let start = Instant::now();

        // 1. Run WASM plugins (if present) → collect WasmCommands → convert to EngineCommands
        let wasm_start = Instant::now();
        if let Some(ref mut runtime) = self.plugin_runtime {
            let wasm_cmds = runtime.run_tick(self.current_tick);
            for wasm_cmd in wasm_cmds {
                if let Some(engine_cmd) = convert_wasm_to_engine(wasm_cmd) {
                    self.commands.push(engine_cmd);
                }
            }
        }
        let wasm_duration = wasm_start.elapsed();

        // 2. Resolve commands (LWW conflict resolution)
        let resolved = self.commands.resolve();
        let command_count = resolved.commands.len();

        // 3. Apply commands
        for cmd in resolved.commands {
            self.apply_command(cmd);
        }

        // 4. Clear command stream for next tick
        self.commands.clear();

        // 5. Drain events (consumed by this tick)
        let _events = self.event_bus.drain_all();

        self.current_tick += 1;
        let duration = start.elapsed();

        observability::TickMetrics {
            tick_number: self.current_tick,
            duration_us: duration.as_micros(),
            command_count,
            entity_count: self.ecs.entity_count(),
            wasm_duration_us: wasm_duration.as_micros(),
        }
    }

    /// Run the tick loop for configured number of ticks (or until max_ticks).
    pub fn run(&mut self) -> Vec<observability::TickMetrics> {
        let mut all_metrics = Vec::new();
        let tick_duration = self.config.tick_duration();

        loop {
            if self.config.max_ticks > 0 && self.current_tick >= self.config.max_ticks {
                break;
            }

            let tick_start = Instant::now();
            let metrics = self.step();
            metrics.log();
            all_metrics.push(metrics);

            // Sleep until next tick
            let elapsed = tick_start.elapsed();
            if elapsed < tick_duration {
                std::thread::sleep(tick_duration - elapsed);
            }
        }

        all_metrics
    }

    fn apply_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::SpawnEntity { tag: _ } => {
                let eid = self.ecs.spawn_entity();
                tracing::debug!(entity = %eid, "spawned entity");
            }
            EngineCommand::DestroyEntity { entity } => {
                let _ = self.space.remove_entity(entity);
                if let Err(e) = self.ecs.despawn_entity(entity) {
                    tracing::warn!(entity = %entity, error = %e, "failed to despawn entity");
                }
            }
            EngineCommand::MoveEntity {
                entity,
                target_room,
            } => {
                if let Err(e) = self.space.move_entity(entity, target_room) {
                    tracing::warn!(
                        entity = %entity,
                        target = %target_room,
                        error = %e,
                        "failed to move entity"
                    );
                }
            }
            EngineCommand::EmitEvent { event_id, payload } => {
                self.event_bus.emit(event_id, payload);
            }
            EngineCommand::SetComponent { .. } | EngineCommand::RemoveComponent { .. } => {
                tracing::trace!("component command applied (no-op in Phase 0/1)");
            }
        }
    }
}

/// Convert a WASM ABI command to an engine-internal command.
fn convert_wasm_to_engine(cmd: WasmCommand) -> Option<EngineCommand> {
    Some(match cmd {
        WasmCommand::SetComponent {
            entity_id,
            component_id,
            data,
        } => EngineCommand::SetComponent {
            entity: EntityId::from_u64(entity_id),
            component_id: ComponentId(component_id),
            data,
        },
        WasmCommand::RemoveComponent {
            entity_id,
            component_id,
        } => EngineCommand::RemoveComponent {
            entity: EntityId::from_u64(entity_id),
            component_id: ComponentId(component_id),
        },
        WasmCommand::EmitEvent { event_id, payload } => EngineCommand::EmitEvent {
            event_id: EventId(event_id),
            payload,
        },
        WasmCommand::SpawnEntity { tag } => EngineCommand::SpawnEntity { tag },
        WasmCommand::DestroyEntity { entity_id } => EngineCommand::DestroyEntity {
            entity: EntityId::from_u64(entity_id),
        },
        WasmCommand::MoveEntity {
            entity_id,
            target_room_id,
        } => EngineCommand::MoveEntity {
            entity: EntityId::from_u64(entity_id),
            target_room: EntityId::from_u64(target_room_id),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use space::RoomGraphSpace;

    #[test]
    fn tick_config_defaults() {
        let config = TickConfig::default();
        assert_eq!(config.tps, 30);
        let dur = config.tick_duration();
        assert!(dur.as_millis() >= 33 && dur.as_millis() <= 34);
    }

    #[test]
    fn single_step() {
        let config = TickConfig {
            tps: 30,
            max_ticks: 1,
        };
        let mut tick_loop = TickLoop::new(config, RoomGraphSpace::new());
        let metrics = tick_loop.step();
        assert_eq!(metrics.tick_number, 1);
        assert_eq!(metrics.command_count, 0);
        assert_eq!(metrics.entity_count, 0);
        assert_eq!(metrics.wasm_duration_us, 0);
    }

    #[test]
    fn backward_compatible_no_plugins() {
        let config = TickConfig {
            tps: 30,
            max_ticks: 10,
        };
        let mut tick_loop = TickLoop::new(config, RoomGraphSpace::new());
        assert!(tick_loop.plugin_runtime.is_none());
        let metrics = tick_loop.run();
        assert_eq!(metrics.len(), 10);
    }

    #[test]
    fn wasm_command_conversion() {
        let wasm_cmd = WasmCommand::MoveEntity {
            entity_id: EntityId::new(1, 0).to_u64(),
            target_room_id: EntityId::new(100, 0).to_u64(),
        };
        let engine_cmd = convert_wasm_to_engine(wasm_cmd).unwrap();
        match engine_cmd {
            EngineCommand::MoveEntity { entity, target_room } => {
                assert_eq!(entity, EntityId::new(1, 0));
                assert_eq!(target_room, EntityId::new(100, 0));
            }
            _ => panic!("expected MoveEntity"),
        }
    }
}
