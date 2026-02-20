use std::path::Path;
use std::time::Duration;

use ecs_adapter::EcsAdapter;
use engine_core::tick::{TickConfig, TickLoop};
use mud::components::*;
use mud::output::{SessionId, SessionOutput};
use mud::parser::{parse_input, PlayerAction};
use mud::persistence_setup::register_mud_components;
use mud::script_setup::register_mud_script_components;
use mud::session::{SessionManager, SessionState};
use mud::systems::{GameContext, PlayerInput};
use net::channels::{NetToTick, OutputTx, PlayerRx};
use net::protocol::{EntityMovedWire, EntityWire, GridConfigWire, ServerMessage};
use persistence::manager::SnapshotManager;
use persistence::registry::PersistenceRegistry;
use persistence::snapshot;
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ScriptConfig;
use space::grid_space::GridConfig;
use space::RoomGraphSpace;
use space::SpaceModel;

const LISTEN_ADDR: &str = "0.0.0.0:4000";
const WS_LISTEN_ADDR: &str = "0.0.0.0:4001";
const SNAPSHOT_INTERVAL: u64 = 300; // Every 300 ticks
const SAVE_DIR: &str = "data/snapshots";
const SCRIPTS_DIR: &str = "scripts";
const GRID_SCRIPTS_DIR: &str = "scripts_grid";
const WEB_STATIC_DIR: &str = "web_dist";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServerMode {
    Mud,
    Grid,
}

fn parse_mode() -> ServerMode {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--mode" {
            if let Some(val) = args.get(i + 1) {
                match val.as_str() {
                    "grid" => return ServerMode::Grid,
                    "mud" => return ServerMode::Mud,
                    other => {
                        eprintln!("Unknown mode '{}', expected 'mud' or 'grid'", other);
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    ServerMode::Mud
}

#[tokio::main]
async fn main() {
    observability::init_logging();

    let mode = parse_mode();
    tracing::info!(?mode, "Rust MUD Engine starting...");

    match mode {
        ServerMode::Mud => run_mud_server().await,
        ServerMode::Grid => run_grid_server().await,
    }
}

async fn run_mud_server() {
    // Channels between async and tick thread
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

    // TCP server
    let register_tx_clone = register_tx.clone();
    let unregister_tx_clone = unregister_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = net::server::run_tcp_server(
            LISTEN_ADDR.to_string(),
            player_tx,
            register_tx_clone,
            unregister_tx_clone,
        )
        .await
        {
            tracing::error!("TCP server error: {}", e);
        }
    });

    tracing::info!("Server listening on {}", LISTEN_ADDR);

    // Tick thread (blocking)
    let tick_handle = std::thread::spawn(move || {
        run_mud_tick_thread(player_rx, output_tx);
    });

    // Wait for tick thread (runs forever)
    let _ = tick_handle.join();
}

async fn run_grid_server() {
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

    // Web server (WebSocket + optional static file serving)
    let register_tx_clone = register_tx.clone();
    let unregister_tx_clone = unregister_tx.clone();
    let static_dir = {
        let p = std::path::PathBuf::from(WEB_STATIC_DIR);
        if p.is_dir() { Some(p) } else { None }
    };
    tokio::spawn(async move {
        if let Err(e) = net::web_server::run_web_server(
            WS_LISTEN_ADDR.to_string(),
            player_tx,
            register_tx_clone,
            unregister_tx_clone,
            static_dir,
        )
        .await
        {
            tracing::error!("Web server error: {}", e);
        }
    });

    tracing::info!("Grid mode web server listening on {}", WS_LISTEN_ADDR);

    // Tick thread (blocking)
    let tick_handle = std::thread::spawn(move || {
        run_grid_tick_thread(player_rx, output_tx);
    });

    // Wait for tick thread (runs forever)
    let _ = tick_handle.join();
}

fn run_grid_tick_thread(mut player_rx: PlayerRx, output_tx: OutputTx) {
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let grid_config = GridConfig {
        width: 256,
        height: 256,
        origin_x: 0,
        origin_y: 0,
    };
    let grid = space::GridSpace::new(grid_config.clone());
    let mut tick_loop = TickLoop::new(config, grid);
    let mut sessions = SessionManager::new();
    let mut aoi = AoiTracker::new(AOI_RADIUS);

    // Initialize scripting engine for grid mode
    let mut script_engine = match ScriptEngine::new(ScriptConfig::default()) {
        Ok(engine) => engine,
        Err(e) => {
            tracing::error!("Failed to initialize script engine: {}", e);
            std::process::exit(1);
        }
    };

    // Load grid scripts (prefer scripts_grid/, fall back to scripts/)
    let grid_scripts_path = Path::new(GRID_SCRIPTS_DIR);
    let scripts_path = Path::new(SCRIPTS_DIR);
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
            // Could be dispatched to on_action hooks in the future
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

const AOI_RADIUS: u32 = 32;

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

// ===== MUD mode functions =====

fn run_mud_tick_thread(mut player_rx: PlayerRx, output_tx: OutputTx) {
    let config = TickConfig {
        tps: 10,
        max_ticks: 0, // unlimited
    };
    let mut tick_loop = TickLoop::new(config, RoomGraphSpace::new());
    let mut sessions = SessionManager::new();
    let snapshot_mgr = SnapshotManager::new(SAVE_DIR);

    // Build persistence registry with MUD components
    let mut registry = PersistenceRegistry::new();
    register_mud_components(&mut registry);

    // Initialize scripting engine
    let mut script_engine = match ScriptEngine::new(ScriptConfig::default()) {
        Ok(engine) => engine,
        Err(e) => {
            tracing::error!("Failed to initialize script engine: {}", e);
            std::process::exit(1);
        }
    };

    // Register MUD components with the script engine
    register_mud_script_components(script_engine.component_registry_mut());

    // Load scripts from scripts/ directory if it exists
    let scripts_path = Path::new(SCRIPTS_DIR);
    if scripts_path.is_dir() {
        match script_engine.load_directory(scripts_path) {
            Ok(()) => {
                tracing::info!(
                    count = script_engine.script_count(),
                    "Loaded Lua scripts"
                );
            }
            Err(e) => {
                tracing::warn!("Failed to load scripts: {}", e);
            }
        }
    } else {
        tracing::info!("No scripts/ directory found, running without Lua scripts");
    }

    // Try to restore from snapshot
    if snapshot_mgr.has_latest() {
        match snapshot_mgr.load_latest() {
            Ok(snap) => {
                match snapshot::restore(snap, &mut tick_loop.ecs, &mut tick_loop.space, &registry) {
                    Ok(tick) => {
                        tick_loop.current_tick = tick;
                        tracing::info!(tick, "Restored from snapshot");
                    }
                    Err(e) => {
                        tracing::warn!("Failed to restore snapshot: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to load snapshot: {}", e);
            }
        }
    }

    // Run on_init hooks (world creation if not restored from snapshot)
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

    // Find spawn room by name (supports both Korean and legacy English)
    let spawn_room = tick_loop
        .ecs
        .entities_with::<Name>()
        .into_iter()
        .find(|&eid| {
            tick_loop
                .ecs
                .get_component::<Name>(eid)
                .map(|n| n.0 == "시작의 방" || n.0 == "Starting Room")
                .unwrap_or(false)
        })
        .expect("시작의 방 not found — ensure scripts/01_world_setup.lua exists");

    let tick_duration = Duration::from_millis(1000 / tick_loop.config.tps as u64);

    loop {
        let tick_start = std::time::Instant::now();

        // 1. Process network messages
        let mut inputs = Vec::new();
        while let Ok(msg) = player_rx.try_recv() {
            match msg {
                NetToTick::NewConnection { session_id } => {
                    handle_new_connection(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        &output_tx,
                        session_id,
                        &script_engine,
                        tick_loop.current_tick,
                    );
                }
                NetToTick::PlayerInput { session_id, line } => {
                    if let Some(input) =
                        handle_player_input(&mut sessions, &output_tx, session_id, &line, spawn_room, &mut tick_loop.ecs, &mut tick_loop.space)
                    {
                        inputs.push(input);
                    }
                }
                NetToTick::Disconnected { session_id } => {
                    handle_disconnect(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        session_id,
                    );
                }
            }
        }

        // 2. Run engine tick (WASM plugins, command stream)
        let _metrics = tick_loop.step();

        // 3. Run game systems — on_action hooks handle player input
        let mut ctx = GameContext {
            ecs: &mut tick_loop.ecs,
            space: &mut tick_loop.space,
            sessions: &sessions,
            tick: tick_loop.current_tick,
        };
        let action_outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&script_engine));
        for output in action_outputs {
            let _ = output_tx.send(output);
        }

        // 4. Run Lua on_tick hooks (combat resolution, periodic systems)
        {
            let mut script_ctx = ScriptContext {
                ecs: &mut tick_loop.ecs,
                space: &mut tick_loop.space,
                sessions: &sessions,
                tick: tick_loop.current_tick,
            };
            match script_engine.run_on_tick(&mut script_ctx) {
                Ok(script_outputs) => {
                    for output in script_outputs {
                        let _ = output_tx.send(output);
                    }
                }
                Err(e) => {
                    tracing::warn!("Lua on_tick error: {}", e);
                }
            }
        }

        // 5. Periodic snapshot
        if tick_loop.current_tick > 0 && tick_loop.current_tick % SNAPSHOT_INTERVAL == 0 {
            let snap =
                snapshot::capture(&tick_loop.ecs, &tick_loop.space, tick_loop.current_tick, &registry);
            if let Err(e) = snapshot_mgr.save_to_disk(&snap) {
                tracing::error!("Failed to save snapshot: {}", e);
            }
        }

        // Sleep for remainder of tick
        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }
}

fn handle_new_connection(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    script_engine: &ScriptEngine,
    tick: u64,
) {
    sessions.create_session_with_id(session_id);
    let _ = output_tx.send(SessionOutput::new(
        session_id,
        "Rust MUD에 오신 것을 환영합니다!\n이름을 입력하세요:",
    ));

    // Fire on_connect hooks
    let mut script_ctx = ScriptContext {
        ecs,
        space,
        sessions,
        tick,
    };
    match script_engine.run_on_connect(&mut script_ctx, session_id) {
        Ok(connect_outputs) => {
            for out in connect_outputs {
                let _ = output_tx.send(out);
            }
        }
        Err(e) => {
            tracing::warn!("Lua on_connect error: {}", e);
        }
    }
}

fn handle_player_input(
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    line: &str,
    spawn_room: ecs_adapter::EntityId,
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
) -> Option<PlayerInput> {
    let session = sessions.get_session(session_id)?;

    match session.state {
        SessionState::AwaitingLogin => {
            let name = line.trim().to_string();
            if name.is_empty() {
                let _ = output_tx.send(SessionOutput::new(session_id, "이름을 입력하세요:"));
                return None;
            }

            // Create player entity
            let entity = ecs.spawn_entity();
            ecs.set_component(entity, Name(name.clone())).unwrap();
            ecs.set_component(entity, PlayerTag).unwrap();
            ecs.set_component(
                entity,
                Health {
                    current: 100,
                    max: 100,
                },
            )
            .unwrap();
            ecs.set_component(entity, Attack(10)).unwrap();
            ecs.set_component(entity, Defense(3)).unwrap();
            ecs.set_component(entity, Inventory::new()).unwrap();
            space.place_entity(entity, spawn_room).unwrap();

            sessions.bind_entity(session_id, entity);
            if let Some(s) = sessions.get_session_mut(session_id) {
                s.player_name = Some(name.clone());
            }

            let _ = output_tx.send(SessionOutput::new(
                session_id,
                format!("환영합니다, {}님!\n'도움말'을 입력하면 명령어를 볼 수 있습니다.", name),
            ));
            // Queue a Look action so the player sees the room on the next tick
            Some(PlayerInput {
                session_id,
                entity,
                action: PlayerAction::Look,
            })
        }
        SessionState::Playing => {
            let entity = session.entity?;
            let action = parse_input(line);

            if action == PlayerAction::Quit {
                let _ = output_tx.send(SessionOutput::new(session_id, "안녕히 가세요!"));
                handle_disconnect(ecs, space, sessions, session_id);
                return None;
            }

            Some(PlayerInput {
                session_id,
                entity,
                action,
            })
        }
        SessionState::Disconnected => None,
    }
}

fn handle_disconnect(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    session_id: SessionId,
) {
    if let Some(entity) = sessions.disconnect(session_id) {
        let _ = space.remove_entity(entity);
        let _ = ecs.despawn_entity(entity);
    }
    sessions.remove_session(session_id);
}
