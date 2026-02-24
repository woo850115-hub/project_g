mod auth_adapter;
mod config;
mod shutdown;

use std::path::Path;
use std::time::Duration;

use ecs_adapter::EcsAdapter;
use engine_core::tick::TickLoop;
use mud::components::*;
use mud::parser::{parse_input, PlayerAction};
use mud::persistence_setup::register_mud_components;
use mud::script_setup::register_mud_script_components;
use mud::systems::{GameContext, PlayerInput};
use net::channels::{NetToTick, OutputTx, PlayerRx};
use persistence::manager::SnapshotManager;
use persistence::registry::PersistenceRegistry;
use persistence::snapshot;
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ContentRegistry;
use session::{SessionId, SessionManager, SessionOutput, SessionState};
use space::RoomGraphSpace;
use space::SpaceModel;

use crate::auth_adapter::PlayerDbAuthProvider;
use crate::config::{parse_cli_args, ServerConfig};
use crate::shutdown::{shutdown_channel, ShutdownRx};

use player_db::PlayerDb;

#[tokio::main]
async fn main() {
    observability::init_logging();

    let config = parse_cli_args();
    tracing::info!("MUD Server starting...");

    let (shutdown_tx, shutdown_rx) = shutdown_channel();

    let config_clone = config.clone();
    let server_future = async move {
        run_mud_server(config_clone, shutdown_rx).await;
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

async fn run_mud_server(config: ServerConfig, shutdown_rx: ShutdownRx) {
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

    // TCP server with shutdown support
    let listen_addr = config.net.telnet_addr.clone();
    let register_tx_clone = register_tx.clone();
    let unregister_tx_clone = unregister_tx.clone();
    let tcp_shutdown = shutdown_rx.clone();
    tokio::spawn(async move {
        if let Err(e) = net::server::run_tcp_server_with_shutdown(
            listen_addr.clone(),
            player_tx,
            register_tx_clone,
            unregister_tx_clone,
            tcp_shutdown.into_inner(),
        )
        .await
        {
            tracing::error!("TCP server error: {}", e);
        }
    });

    tracing::info!("Server listening on {}", config.net.telnet_addr);

    // Tick thread (blocking)
    let tick_shutdown = shutdown_rx;
    let tick_handle = std::thread::spawn(move || {
        run_mud_tick_thread(player_rx, output_tx, config, tick_shutdown);
    });

    // Wait for tick thread
    let _ = tick_handle.join();
}

fn run_mud_tick_thread(mut player_rx: PlayerRx, output_tx: OutputTx, config: ServerConfig, shutdown_rx: ShutdownRx) {
    let tick_config = config.to_tick_config();
    let mut tick_loop = TickLoop::new(tick_config, RoomGraphSpace::new());
    let mut sessions = SessionManager::new();
    let snapshot_mgr = SnapshotManager::new(&config.persistence.save_dir);
    let auth_required = config.database.auth_required;

    // Open player DB if auth is required
    let player_db = if auth_required {
        match PlayerDb::open(&config.database.path) {
            Ok(db) => {
                tracing::info!(path = %config.database.path, "Player database opened");
                Some(db)
            }
            Err(e) => {
                tracing::error!("Failed to open player database: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Build persistence registry with MUD components
    let mut registry = PersistenceRegistry::new();
    register_mud_components(&mut registry);

    // Initialize scripting engine
    let mut script_engine = match ScriptEngine::new(config.to_script_config()) {
        Ok(engine) => engine,
        Err(e) => {
            tracing::error!("Failed to initialize script engine: {}", e);
            std::process::exit(1);
        }
    };

    // Register MUD components with the script engine
    register_mud_script_components(script_engine.component_registry_mut());

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

    // Load scripts from scripts/ directory if it exists
    let scripts_path = Path::new(&config.scripting.scripts_dir);
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
            sessions: &mut sessions,
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
    let snapshot_interval = config.persistence.snapshot_interval;
    let character_save_interval = config.character.save_interval;
    let linger_timeout_ticks = config.character.linger_timeout_secs * config.tick.tps as u64;

    loop {
        if shutdown_rx.is_shutdown() {
            tracing::info!("MUD tick loop: shutdown signal received");
            // Save all characters to DB before shutdown
            if let Some(ref db) = player_db {
                auto_save_characters(&tick_loop.ecs, &tick_loop.space, &sessions, db);
                // Also save lingering entities
                for linger in sessions.lingering_entities() {
                    save_character_state(
                        &tick_loop.ecs,
                        &tick_loop.space,
                        linger.entity,
                        linger.character_id,
                        db,
                    );
                }
            }
            // Send shutdown message to all connected sessions
            for session in sessions.playing_sessions() {
                let _ = output_tx.send(SessionOutput::with_disconnect(
                    session.session_id,
                    "서버가 종료됩니다. 안녕히 가세요!",
                ));
            }
            // Final snapshot save
            let snap = snapshot::capture(
                &tick_loop.ecs,
                &tick_loop.space,
                tick_loop.current_tick,
                &registry,
            );
            if let Err(e) = snapshot_mgr.save_to_disk(&snap) {
                tracing::error!("Failed to save final snapshot: {}", e);
            } else {
                tracing::info!(tick = tick_loop.current_tick, "Final snapshot saved");
            }
            break;
        }

        let tick_start = std::time::Instant::now();

        // Build auth provider for this tick (if auth is enabled)
        let auth_provider = player_db.as_ref().map(|db| PlayerDbAuthProvider::new(db));

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
                        auth_provider.as_ref().map(|p| p as &dyn scripting::AuthProvider),
                    );
                }
                NetToTick::PlayerInput { session_id, line } => {
                    if let Some(input) = handle_player_input(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        &output_tx,
                        session_id,
                        &line,
                        &script_engine,
                        tick_loop.current_tick,
                        auth_provider.as_ref().map(|p| p as &dyn scripting::AuthProvider),
                    ) {
                        inputs.push(input);
                    }
                }
                NetToTick::Disconnected { session_id } => {
                    handle_disconnect(
                        &mut tick_loop.ecs,
                        &mut tick_loop.space,
                        &mut sessions,
                        &output_tx,
                        session_id,
                        &script_engine,
                        tick_loop.current_tick,
                        auth_provider.as_ref().map(|p| p as &dyn scripting::AuthProvider),
                    );
                }
            }
        }

        // 2. Run engine tick (WASM plugins, command stream)
        let _metrics = tick_loop.step();

        // 3. Separate admin commands from normal inputs
        let mut normal_inputs = Vec::new();
        let mut admin_inputs = Vec::new();
        for input in inputs {
            if let PlayerAction::Admin { ref command, ref args } = input.action {
                admin_inputs.push((input.session_id, input.entity, command.clone(), args.clone()));
            } else {
                normal_inputs.push(input);
            }
        }

        // 3a. Run game systems — on_action hooks handle player input
        let mut ctx = GameContext {
            ecs: &mut tick_loop.ecs,
            space: &mut tick_loop.space,
            sessions: &mut sessions,
            tick: tick_loop.current_tick,
        };
        let action_outputs = mud::systems::run_game_systems(&mut ctx, normal_inputs, Some(&script_engine));
        for output in action_outputs {
            let _ = output_tx.send(output);
        }

        // 3b. Run admin commands via on_admin hooks
        for (admin_sid, admin_entity, admin_cmd, admin_args) in admin_inputs {
            let permission = sessions
                .get_session(admin_sid)
                .map(|s| s.permission.as_i32())
                .unwrap_or(0);
            let admin_info = scripting::engine::AdminInfo {
                command: admin_cmd.clone(),
                args: admin_args,
                session_id: admin_sid,
                entity: admin_entity,
                permission,
            };
            let mut script_ctx = ScriptContext {
                ecs: &mut tick_loop.ecs,
                space: &mut tick_loop.space,
                sessions: &mut sessions,
                tick: tick_loop.current_tick,
            };
            match script_engine.run_on_admin(&mut script_ctx, &admin_info) {
                Ok((admin_outputs, handled)) => {
                    for out in admin_outputs {
                        let _ = output_tx.send(out);
                    }
                    if !handled {
                        if permission < 1 {
                            let _ = output_tx.send(SessionOutput::new(
                                admin_sid,
                                "관리자 명령어를 사용할 권한이 없습니다.",
                            ));
                        } else {
                            let _ = output_tx.send(SessionOutput::new(
                                admin_sid,
                                format!("알 수 없는 관리자 명령어: /{}", admin_cmd),
                            ));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Admin command error: {}", e);
                    let _ = output_tx.send(SessionOutput::new(
                        admin_sid,
                        format!("관리자 명령어 오류: {}", e),
                    ));
                }
            }
        }

        // 4. Run Lua on_tick hooks (combat resolution, periodic systems)
        {
            let mut script_ctx = ScriptContext {
                ecs: &mut tick_loop.ecs,
                space: &mut tick_loop.space,
                sessions: &mut sessions,
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
        if tick_loop.current_tick > 0 && tick_loop.current_tick % snapshot_interval == 0 {
            let snap =
                snapshot::capture(&tick_loop.ecs, &tick_loop.space, tick_loop.current_tick, &registry);
            if let Err(e) = snapshot_mgr.save_to_disk(&snap) {
                tracing::error!("Failed to save snapshot: {}", e);
            }
        }

        // 6. Character auto-save (only in auth mode)
        if let Some(ref db) = player_db {
            if character_save_interval > 0
                && tick_loop.current_tick > 0
                && tick_loop.current_tick % character_save_interval == 0
            {
                auto_save_characters(&tick_loop.ecs, &tick_loop.space, &sessions, db);
            }

            // 7. Clean up expired lingering entities
            if linger_timeout_ticks > 0 {
                cleanup_expired_lingering(
                    &mut tick_loop.ecs,
                    &mut tick_loop.space,
                    &mut sessions,
                    tick_loop.current_tick,
                    linger_timeout_ticks,
                    Some(db),
                );
            }
        }

        // Sleep for remainder of tick
        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            std::thread::sleep(tick_duration - elapsed);
        }
    }

    tracing::info!("MUD tick loop stopped");
}

fn handle_new_connection(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    script_engine: &ScriptEngine,
    tick: u64,
    auth: Option<&dyn scripting::AuthProvider>,
) {
    sessions.create_session_with_id(session_id);

    // Fire on_connect hooks (Lua sends welcome message)
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

    // If on_connect didn't send anything (no hooks registered), send a default
    // This is only for backwards compatibility when no login script exists
    let _ = auth; // auth not needed here; on_connect doesn't require it
}

fn handle_player_input(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    line: &str,
    script_engine: &ScriptEngine,
    current_tick: u64,
    auth: Option<&dyn scripting::AuthProvider>,
) -> Option<PlayerInput> {
    let session = sessions.get_session(session_id)?;
    let state = session.state.clone();

    match state {
        SessionState::Login => {
            // Delegate all login logic to Lua via on_input hooks
            let mut script_ctx = ScriptContext {
                ecs,
                space,
                sessions,
                tick: current_tick,
            };
            match script_engine.run_on_input(&mut script_ctx, session_id, line, auth) {
                Ok(input_outputs) => {
                    for out in input_outputs {
                        let _ = output_tx.send(out);
                    }
                }
                Err(e) => {
                    tracing::warn!("Lua on_input error: {}", e);
                }
            }

            // Check if Lua transitioned the session to Playing
            if let Some(session) = sessions.get_session(session_id) {
                if session.state == SessionState::Playing {
                    if let Some(entity) = session.entity {
                        // Auto-look after login
                        return Some(PlayerInput {
                            session_id,
                            entity,
                            action: PlayerAction::Look,
                        });
                    }
                }
            }

            None
        }
        SessionState::Playing => {
            let entity = session.entity?;
            let action = parse_input(line);

            if action == PlayerAction::Quit {
                let _ = output_tx.send(SessionOutput::with_disconnect(session_id, "안녕히 가세요!"));
                handle_disconnect(ecs, space, sessions, output_tx, session_id, script_engine, current_tick, auth);
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
    output_tx: &OutputTx,
    session_id: SessionId,
    script_engine: &ScriptEngine,
    current_tick: u64,
    auth: Option<&dyn scripting::AuthProvider>,
) {
    // Fire on_disconnect hooks (Lua handles save/linger/despawn)
    let mut script_ctx = ScriptContext {
        ecs,
        space,
        sessions,
        tick: current_tick,
    };
    match script_engine.run_on_disconnect(&mut script_ctx, session_id, auth) {
        Ok(disconnect_outputs) => {
            for out in disconnect_outputs {
                let _ = output_tx.send(out);
            }
        }
        Err(e) => {
            tracing::warn!("Lua on_disconnect error: {}", e);
        }
    }

    // Fallback cleanup: if Lua didn't handle everything, clean up here.
    // This ensures resources are freed even if there's no on_disconnect hook.
    if sessions.get_session(session_id).is_some() {
        if let Some(entity) = sessions.disconnect(session_id) {
            let _ = space.remove_entity(entity);
            let _ = ecs.despawn_entity(entity);
        }
        sessions.remove_session(session_id);
    }
}

/// Save a single character's ECS state to the database.
fn save_character_state(
    ecs: &EcsAdapter,
    space: &RoomGraphSpace,
    entity: ecs_adapter::EntityId,
    character_id: i64,
    db: &PlayerDb,
) {
    let mut components = serde_json::Map::new();

    if let Ok(health) = ecs.get_component::<Health>(entity) {
        components.insert(
            "Health".to_string(),
            serde_json::json!({"current": health.current, "max": health.max}),
        );
    }
    if let Ok(attack) = ecs.get_component::<Attack>(entity) {
        components.insert("Attack".to_string(), serde_json::json!(attack.0));
    }
    if let Ok(defense) = ecs.get_component::<Defense>(entity) {
        components.insert("Defense".to_string(), serde_json::json!(defense.0));
    }
    if let Ok(race) = ecs.get_component::<Race>(entity) {
        components.insert("Race".to_string(), serde_json::json!(race.0));
    }
    if let Ok(class) = ecs.get_component::<Class>(entity) {
        components.insert("Class".to_string(), serde_json::json!(class.0));
    }
    if let Ok(level) = ecs.get_component::<Level>(entity) {
        components.insert("Level".to_string(), serde_json::json!(level.0));
    }
    if let Ok(exp) = ecs.get_component::<Experience>(entity) {
        components.insert("Experience".to_string(), serde_json::json!(exp.0));
    }
    if let Ok(mana) = ecs.get_component::<Mana>(entity) {
        components.insert(
            "Mana".to_string(),
            serde_json::json!({"current": mana.current, "max": mana.max}),
        );
    }
    if let Ok(skills) = ecs.get_component::<Skills>(entity) {
        components.insert(
            "Skills".to_string(),
            serde_json::json!({"learned": skills.learned}),
        );
    }

    let room_id = space.entity_room(entity).map(|r| r.to_u64());

    if let Err(e) = db.character().save_state(
        character_id,
        &serde_json::Value::Object(components),
        room_id,
        None,
    ) {
        tracing::warn!(character_id, "Failed to save character state: {}", e);
    }
}

/// Auto-save all playing characters to DB.
fn auto_save_characters(
    ecs: &EcsAdapter,
    space: &RoomGraphSpace,
    sessions: &SessionManager,
    db: &PlayerDb,
) {
    let mut count = 0u32;
    for session in sessions.playing_sessions() {
        if let (Some(entity), Some(character_id)) = (session.entity, session.character_id) {
            save_character_state(ecs, space, entity, character_id, db);
            count += 1;
        }
    }
    if count > 0 {
        tracing::info!(count, "Auto-saved character states");
    }
}

/// Clean up expired lingering entities.
fn cleanup_expired_lingering(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    current_tick: u64,
    timeout_ticks: u64,
    db: Option<&PlayerDb>,
) {
    let expired = sessions.expired_lingering(current_tick, timeout_ticks);
    for character_id in expired {
        if let Some(linger) = sessions.remove_lingering(character_id) {
            // Save final state to DB before despawning
            if let Some(db) = db {
                save_character_state(ecs, space, linger.entity, linger.character_id, db);
            }
            let _ = space.remove_entity(linger.entity);
            let _ = ecs.despawn_entity(linger.entity);
            tracing::info!(character_id, ?linger.entity, "Lingering entity expired, despawned");
        }
    }
}
