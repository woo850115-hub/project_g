mod config;
mod shutdown;

use std::path::Path;
use std::time::Duration;

use ecs_adapter::EcsAdapter;
use engine_core::tick::TickLoop;
use net::channels::{NetToTick, OutputTx, PlayerRx};
use net::protocol::{EntityMovedWire, EntityWire, GridConfigWire, ServerMessage};
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ContentRegistry;
use session::{SessionId, SessionManager, SessionOutput, SessionState};
use space::grid_space::GridConfig;
use space::SpaceModel;

use crate::config::{parse_cli_args, ServerConfig};
use crate::shutdown::{shutdown_channel, ShutdownRx};

pub use project_2d::components::Name;

#[tokio::main]
async fn main() {
    observability::init_logging();

    let config = parse_cli_args();
    tracing::info!("Grid Server starting...");

    let (shutdown_tx, shutdown_rx) = shutdown_channel();

    let config_clone = config.clone();
    let server_future = async move {
        run_grid_server(config_clone, shutdown_rx).await;
    };

    tokio::select! {
        _ = shutdown::wait_for_signal() => {
            tracing::info!("Shutdown signal received, stopping server...");
            shutdown_tx.trigger();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        _ = server_future => {}
    }

    tracing::info!("Server stopped.");
}

async fn run_grid_server(config: ServerConfig, shutdown_rx: ShutdownRx) {
    // Channels between async and tick thread (same pattern as MUD mode)
    let (player_tx, player_rx) = tokio::sync::mpsc::unbounded_channel();
    let (output_tx, output_rx) = tokio::sync::mpsc::unbounded_channel();
    let (register_tx, register_rx) = tokio::sync::mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = tokio::sync::mpsc::unbounded_channel();

    // Output router
    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    // Web server with shutdown support
    let ws_addr = config.net.ws_addr.clone();
    let register_tx_clone = register_tx.clone();
    let unregister_tx_clone = unregister_tx.clone();
    let static_dir = {
        let p = std::path::PathBuf::from(&config.net.web_static_dir);
        if p.is_dir() { Some(p) } else { None }
    };
    let ws_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        if let Err(e) = net::web_server::run_web_server_with_shutdown(
            ws_addr,
            player_tx,
            register_tx_clone,
            unregister_tx_clone,
            static_dir,
            ws_shutdown.into_inner(),
        )
        .await
        {
            tracing::error!("Web server error: {}", e);
        }
    });

    tracing::info!("Grid mode web server listening on {}", config.net.ws_addr);

    // Tick thread (blocking)
    let tick_shutdown = shutdown_rx;
    let tick_handle = std::thread::spawn(move || {
        run_grid_tick_thread(player_rx, output_tx, config, tick_shutdown);
    });

    // Wait for tick thread
    let _ = tick_handle.join();
}

fn run_grid_tick_thread(mut player_rx: PlayerRx, output_tx: OutputTx, config: ServerConfig, shutdown_rx: ShutdownRx) {
    let tick_config = config.to_tick_config();
    let grid_config = config.to_grid_config();
    let grid = space::GridSpace::new(grid_config.clone());
    let mut tick_loop = TickLoop::new(tick_config, grid);
    let mut sessions = SessionManager::new();
    let mut aoi = AoiTracker::new(config.grid.aoi_radius);

    // Initialize scripting engine for grid mode
    let mut script_engine = match ScriptEngine::new(config.to_script_config()) {
        Ok(engine) => engine,
        Err(e) => {
            tracing::error!("Failed to initialize script engine: {}", e);
            std::process::exit(1);
        }
    };

    // Load content from content/ directory if it exists
    let content_path = Path::new(&config.scripting.content_dir);
    if content_path.is_dir() {
        match ContentRegistry::load_dir(content_path) {
            Ok(registry) => {
                tracing::info!(
                    collections = registry.collection_names().len(),
                    items = registry.total_count(),
                    "Content loaded"
                );
                if let Err(e) = script_engine.register_content(&registry) {
                    tracing::warn!("Failed to register content in Lua: {}", e);
                }
            }
            Err(e) => tracing::warn!("Failed to load content: {}", e),
        }
    }

    // Load grid scripts (prefer grid_scripts_dir, fall back to scripts_dir)
    let grid_scripts_path = Path::new(&config.scripting.grid_scripts_dir);
    let scripts_path = Path::new(&config.scripting.scripts_dir);
    let load_path = if grid_scripts_path.is_dir() {
        Some(grid_scripts_path)
    } else if scripts_path.is_dir() {
        Some(scripts_path)
    } else {
        None
    };

    if let Some(dir) = load_path {
        match script_engine.load_directory(dir) {
            Ok(()) => {
                tracing::info!(
                    count = script_engine.script_count(),
                    dir = %dir.display(),
                    "Loaded grid Lua scripts"
                );
            }
            Err(e) => {
                tracing::warn!("Failed to load grid scripts: {}", e);
            }
        }
    } else {
        tracing::info!("No scripts_grid/ or scripts/ directory found, running without Lua scripts");
    }

    // Run on_init hooks
    {
        let mut script_ctx = ScriptContext {
            ecs: &mut tick_loop.ecs,
            space: &mut tick_loop.space,
            sessions: &sessions,
            tick: tick_loop.current_tick,
        };
        match script_engine.run_on_init(&mut script_ctx) {
            Ok(init_outputs) => {
                for out in init_outputs {
                    let _ = output_tx.send(out);
                }
            }
            Err(e) => {
                tracing::error!("Lua on_init error: {}", e);
            }
        }
    }

    let tick_duration = Duration::from_millis(1000 / tick_loop.config.tps as u64);

    tracing::info!("Grid tick loop running (Ctrl+C to stop)");

    loop {
        if shutdown_rx.is_shutdown() {
            tracing::info!("Grid tick loop: shutdown signal received");
            // Send shutdown message to all connected sessions
            for session in sessions.playing_sessions() {
                let _ = output_tx.send(SessionOutput::with_disconnect(
                    session.session_id,
                    serde_json::to_string(&ServerMessage::Error {
                        message: "Server is shutting down.".to_string(),
                    })
                    .unwrap(),
                ));
            }
            break;
        }

        let tick_start = std::time::Instant::now();

        // 1. Process network messages
        while let Ok(msg) = player_rx.try_recv() {
            match msg {
                NetToTick::NewConnection { session_id } => {
                    handle_grid_new_connection(&mut sessions, &output_tx, session_id);
                }
                NetToTick::PlayerInput { session_id, line } => {
                    handle_grid_player_input(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        &output_tx,
                        session_id,
                        &line,
                        &grid_config,
                        tick_loop.current_tick,
                        &mut aoi,
                    );
                }
                NetToTick::Disconnected { session_id } => {
                    handle_grid_disconnect(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        session_id,
                        &mut aoi,
                    );
                }
            }
        }

        // 2. Run engine tick (WASM plugins, command stream)
        let _metrics = tick_loop.step();

        // 3. Run Lua on_tick hooks
        {
            let mut script_ctx = ScriptContext {
                ecs: &mut tick_loop.ecs,
                space: &mut tick_loop.space,
                sessions: &sessions,
                tick: tick_loop.current_tick,
            };
            match script_engine.run_on_tick(&mut script_ctx) {
                Ok(script_outputs) => {
                    for out in script_outputs {
                        let _ = output_tx.send(out);
                    }
                }
                Err(e) => {
                    tracing::warn!("Lua on_tick error: {}", e);
                }
            }
        }

        // 4. Broadcast delta to all playing sessions (AOI filtering)
        broadcast_delta(
            &tick_loop.ecs,
            &tick_loop.space,
            &sessions,
            &output_tx,
            tick_loop.current_tick,
            &mut aoi,
        );

        // Sleep for remainder of tick
        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }

    tracing::info!("Grid tick loop stopped");
}

fn handle_grid_new_connection(
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
) {
    sessions.create_session_with_id(session_id);
    tracing::info!(?session_id, "Grid: new connection (awaiting login)");
    // No welcome message yet — client sends Connect with name
    let _ = output_tx;
}

fn handle_grid_player_input(
    ecs: &mut EcsAdapter,
    space: &mut space::GridSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    line: &str,
    grid_config: &GridConfig,
    tick: u64,
    aoi: &mut AoiTracker,
) {
    let state = match sessions.get_session(session_id) {
        Some(s) => s.state.clone(),
        None => return,
    };

    match state {
        SessionState::AwaitingLogin => {
            let name = line.trim().to_string();
            if name.is_empty() {
                return;
            }

            // Spawn player entity at grid center
            let entity = ecs.spawn_entity();
            let center_x = grid_config.origin_x + (grid_config.width as i32) / 2;
            let center_y = grid_config.origin_y + (grid_config.height as i32) / 2;
            ecs.set_component(entity, Name(name.clone())).unwrap();
            if let Err(e) = space.set_position(entity, center_x, center_y) {
                tracing::error!(?entity, "Failed to place entity on grid: {}", e);
                let _ = ecs.despawn_entity(entity);
                let err_msg = ServerMessage::Error {
                    message: format!("Failed to spawn: {}", e),
                };
                let _ = output_tx.send(SessionOutput::new(
                    session_id,
                    serde_json::to_string(&err_msg).unwrap(),
                ));
                return;
            }

            sessions.bind_entity(session_id, entity);
            if let Some(s) = sessions.get_session_mut(session_id) {
                s.player_name = Some(name);
            }
            aoi.on_session_playing(session_id);

            // Send Welcome message
            let welcome = ServerMessage::Welcome {
                session_id: session_id.0,
                entity_id: entity.to_u64(),
                tick,
                grid_config: GridConfigWire {
                    width: grid_config.width,
                    height: grid_config.height,
                    origin_x: grid_config.origin_x,
                    origin_y: grid_config.origin_y,
                },
            };
            let _ = output_tx.send(SessionOutput::new(
                session_id,
                serde_json::to_string(&welcome).unwrap(),
            ));

            tracing::info!(?session_id, ?entity, "Grid: player spawned");
        }
        SessionState::AwaitingPassword { .. }
        | SessionState::AwaitingPasswordConfirm { .. }
        | SessionState::SelectingCharacter { .. } => {
            // Grid mode doesn't use auth flow — ignore
        }
        SessionState::Playing => {
            let entity = match sessions.get_session(session_id).and_then(|s| s.entity) {
                Some(e) => e,
                None => return,
            };

            if line == "__ping" {
                let pong = ServerMessage::Pong;
                let _ = output_tx.send(SessionOutput::new(
                    session_id,
                    serde_json::to_string(&pong).unwrap(),
                ));
                return;
            }

            if let Some(rest) = line.strip_prefix("__grid_move ") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.len() == 2 {
                    if let (Ok(dx), Ok(dy)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                        if let Some(pos) = space.get_position(entity) {
                            let new_x = pos.x + dx;
                            let new_y = pos.y + dy;
                            if let Err(e) = space.move_to(entity, new_x, new_y) {
                                let err_msg = ServerMessage::Error {
                                    message: format!("{}", e),
                                };
                                let _ = output_tx.send(SessionOutput::new(
                                    session_id,
                                    serde_json::to_string(&err_msg).unwrap(),
                                ));
                            }
                        }
                    }
                }
                return;
            }

            // Generic action passthrough (for Lua hooks)
            tracing::debug!(?session_id, line, "Grid: unhandled player input");
        }
        SessionState::Disconnected => {}
    }
}

fn handle_grid_disconnect(
    ecs: &mut EcsAdapter,
    space: &mut space::GridSpace,
    sessions: &mut SessionManager,
    session_id: SessionId,
    aoi: &mut AoiTracker,
) {
    if let Some(entity) = sessions.disconnect(session_id) {
        let _ = space.remove_entity(entity);
        let _ = ecs.despawn_entity(entity);
    }
    aoi.on_session_removed(session_id);
    sessions.remove_session(session_id);
}

struct SessionAoiState {
    known: std::collections::BTreeMap<ecs_adapter::EntityId, space::grid_space::GridPos>,
}

struct AoiTracker {
    sessions: std::collections::BTreeMap<SessionId, SessionAoiState>,
    radius: u32,
}

impl AoiTracker {
    fn new(radius: u32) -> Self {
        Self {
            sessions: std::collections::BTreeMap::new(),
            radius,
        }
    }

    fn on_session_playing(&mut self, session_id: SessionId) {
        self.sessions.insert(
            session_id,
            SessionAoiState {
                known: std::collections::BTreeMap::new(),
            },
        );
    }

    fn on_session_removed(&mut self, session_id: SessionId) {
        self.sessions.remove(&session_id);
    }
}

fn broadcast_delta(
    ecs: &EcsAdapter,
    space: &space::GridSpace,
    sessions: &SessionManager,
    output_tx: &OutputTx,
    tick: u64,
    aoi: &mut AoiTracker,
) {
    let playing = sessions.playing_sessions();
    if playing.is_empty() {
        return;
    }

    let all_positions = space.all_entity_positions();

    // Name cache to avoid repeated ECS lookups
    let mut name_cache: std::collections::BTreeMap<ecs_adapter::EntityId, Option<String>> =
        std::collections::BTreeMap::new();

    for session in &playing {
        let self_entity = match session.entity {
            Some(e) => e,
            None => continue,
        };
        let player_pos = match space.get_position(self_entity) {
            Some(p) => p,
            None => continue,
        };

        let aoi_state = match aoi.sessions.get_mut(&session.session_id) {
            Some(s) => s,
            None => continue,
        };

        // Current entities in AOI
        let in_radius = space.entities_in_radius(player_pos.x, player_pos.y, aoi.radius);
        let current_aoi: std::collections::BTreeMap<ecs_adapter::EntityId, space::grid_space::GridPos> =
            in_radius
                .into_iter()
                .filter_map(|eid| {
                    all_positions.get(&eid).map(|pos| (eid, *pos))
                })
                .collect();

        // Compute delta
        let mut entered = Vec::new();
        let mut moved = Vec::new();
        let mut left = Vec::new();

        // Check for left: in known but not in current AOI
        for (eid, _) in aoi_state.known.iter() {
            if !current_aoi.contains_key(eid) {
                left.push(eid.to_u64());
            }
        }

        // Check for entered and moved
        for (&eid, &pos) in &current_aoi {
            match aoi_state.known.get(&eid) {
                None => {
                    // New entity in AOI — entered
                    let name = name_cache
                        .entry(eid)
                        .or_insert_with(|| {
                            ecs.get_component::<Name>(eid).ok().map(|n| n.0.clone())
                        })
                        .clone();
                    entered.push(EntityWire {
                        id: eid.to_u64(),
                        x: pos.x,
                        y: pos.y,
                        name,
                        is_self: eid == self_entity,
                    });
                }
                Some(old_pos) => {
                    if old_pos.x != pos.x || old_pos.y != pos.y {
                        // Position changed — moved
                        moved.push(EntityMovedWire {
                            id: eid.to_u64(),
                            x: pos.x,
                            y: pos.y,
                        });
                    }
                }
            }
        }

        // Update known state
        aoi_state.known = current_aoi;

        // Send StateDelta
        let delta = ServerMessage::StateDelta {
            tick,
            entered,
            moved,
            left,
        };
        let _ = output_tx.send(SessionOutput::new(
            session.session_id,
            serde_json::to_string(&delta).unwrap(),
        ));
    }
}
