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
                        handle_player_input(&mut sessions, &output_tx, session_id, &line, spawn_room, &mut tick_loop.ecs, &mut tick_loop.space, player_db.as_ref(), tick_loop.current_tick)
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
                        player_db.as_ref(),
                        tick_loop.current_tick,
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
            sessions: &sessions,
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
                sessions: &sessions,
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
    player_db: Option<&PlayerDb>,
    current_tick: u64,
) -> Option<PlayerInput> {
    let session = sessions.get_session(session_id)?;
    let state = session.state.clone();

    match state {
        SessionState::AwaitingLogin => {
            let name = line.trim().to_string();
            if name.is_empty() {
                let _ = output_tx.send(SessionOutput::new(session_id, "이름을 입력하세요:"));
                return None;
            }

            if let Some(db) = player_db {
                // Auth mode: check if account exists
                let exists = db.account().get_by_username(&name).ok().flatten().is_some();
                if let Some(s) = sessions.get_session_mut(session_id) {
                    s.state = SessionState::AwaitingPassword {
                        username: name.clone(),
                        is_new: !exists,
                    };
                }
                if exists {
                    let _ = output_tx.send(SessionOutput::new(session_id, "비밀번호를 입력하세요:"));
                } else {
                    let _ = output_tx.send(SessionOutput::new(
                        session_id,
                        format!("'{}' — 새 계정입니다. 비밀번호를 설정하세요:", name),
                    ));
                }
                return None;
            }

            // Quick-play mode: create entity immediately
            spawn_player_entity(ecs, space, sessions, output_tx, session_id, &name, spawn_room)
        }
        SessionState::AwaitingPassword { ref username, is_new } => {
            let password = line.trim().to_string();
            if password.is_empty() {
                let _ = output_tx.send(SessionOutput::new(session_id, "비밀번호를 입력하세요:"));
                return None;
            }

            let db = player_db?;

            if is_new {
                // New account: confirm password
                if let Some(s) = sessions.get_session_mut(session_id) {
                    s.state = SessionState::AwaitingPasswordConfirm {
                        username: username.clone(),
                        password,
                    };
                }
                let _ = output_tx.send(SessionOutput::new(session_id, "비밀번호를 다시 입력하세요:"));
                return None;
            }

            // Existing account: authenticate
            match db.account().authenticate(username, &password) {
                Ok(account) => {
                    enter_character_selection(sessions, output_tx, session_id, &account, db);
                }
                Err(player_db::PlayerDbError::InvalidPassword) => {
                    let _ = output_tx.send(SessionOutput::new(session_id, "비밀번호가 틀렸습니다. 비밀번호를 입력하세요:"));
                }
                Err(e) => {
                    tracing::warn!("Auth error: {}", e);
                    let _ = output_tx.send(SessionOutput::new(session_id, "인증 오류가 발생했습니다. 이름을 입력하세요:"));
                    if let Some(s) = sessions.get_session_mut(session_id) {
                        s.state = SessionState::AwaitingLogin;
                    }
                }
            }
            None
        }
        SessionState::AwaitingPasswordConfirm { ref username, ref password } => {
            let confirm = line.trim().to_string();
            if confirm != *password {
                let _ = output_tx.send(SessionOutput::new(session_id, "비밀번호가 일치하지 않습니다. 이름을 입력하세요:"));
                if let Some(s) = sessions.get_session_mut(session_id) {
                    s.state = SessionState::AwaitingLogin;
                }
                return None;
            }

            let db = player_db?;
            let username = username.clone();
            let password = password.clone();

            match db.account().create(&username, &password) {
                Ok(account) => {
                    let _ = output_tx.send(SessionOutput::new(session_id, "계정이 생성되었습니다!"));
                    enter_character_selection(sessions, output_tx, session_id, &account, db);
                }
                Err(e) => {
                    tracing::warn!("Account creation error: {}", e);
                    let _ = output_tx.send(SessionOutput::new(session_id, "계정 생성에 실패했습니다. 이름을 입력하세요:"));
                    if let Some(s) = sessions.get_session_mut(session_id) {
                        s.state = SessionState::AwaitingLogin;
                    }
                }
            }
            None
        }
        SessionState::SelectingCharacter { account_id, permission } => {
            let choice = line.trim();
            let db = player_db?;

            let chars = db.character().list_for_account(account_id).ok()?;
            let new_index = chars.len() + 1;

            if choice == new_index.to_string() || choice.eq_ignore_ascii_case("new") {
                // Create new character — use username as default name
                let session = sessions.get_session(session_id)?;
                let default_name = session.player_name.clone().unwrap_or_else(|| "모험자".to_string());
                let defaults = serde_json::json!({
                    "Health": {"current": 100, "max": 100},
                    "Attack": 10,
                    "Defense": 3
                });
                match db.character().create(account_id, &default_name, &defaults) {
                    Ok(character) => {
                        return spawn_character_from_db(
                            ecs, space, sessions, output_tx, session_id,
                            &character, spawn_room, permission,
                        );
                    }
                    Err(e) => {
                        let _ = output_tx.send(SessionOutput::new(
                            session_id,
                            format!("캐릭터 생성 실패: {}. 다시 선택하세요:", e),
                        ));
                    }
                }
                return None;
            }

            // Parse numeric choice
            if let Ok(num) = choice.parse::<usize>() {
                if num >= 1 && num <= chars.len() {
                    let character = &chars[num - 1];
                    return spawn_character_from_db(
                        ecs, space, sessions, output_tx, session_id,
                        character, spawn_room, permission,
                    );
                }
            }

            let _ = output_tx.send(SessionOutput::new(session_id, "잘못된 선택입니다. 번호를 입력하세요:"));
            None
        }
        SessionState::Playing => {
            let entity = session.entity?;
            let action = parse_input(line);

            if action == PlayerAction::Quit {
                let _ = output_tx.send(SessionOutput::with_disconnect(session_id, "안녕히 가세요!"));
                handle_disconnect(ecs, space, sessions, session_id, player_db, current_tick);
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

fn enter_character_selection(
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    account: &player_db::Account,
    db: &PlayerDb,
) {
    if let Some(s) = sessions.get_session_mut(session_id) {
        s.account_id = Some(account.id);
        s.player_name = Some(account.username.clone());
        s.permission = session::PermissionLevel::from_i32(account.permission.as_i32());
        s.state = SessionState::SelectingCharacter {
            account_id: account.id,
            permission: session::PermissionLevel::from_i32(account.permission.as_i32()),
        };
    }

    let chars = db
        .character()
        .list_for_account(account.id)
        .unwrap_or_default();

    let mut msg = String::from("캐릭터를 선택하세요:\n");
    for (i, c) in chars.iter().enumerate() {
        msg.push_str(&format!("  {}. {}\n", i + 1, c.name));
    }
    msg.push_str(&format!("  {}. [새 캐릭터]\n선택:", chars.len() + 1));

    let _ = output_tx.send(SessionOutput::new(session_id, msg));
}

fn spawn_character_from_db(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    character: &player_db::CharacterRecord,
    spawn_room: ecs_adapter::EntityId,
    permission: session::PermissionLevel,
) -> Option<PlayerInput> {
    // Check for lingering entity (seamless reconnection)
    if let Some(entity) = sessions.rebind_lingering(session_id, character.id) {
        if let Some(s) = sessions.get_session_mut(session_id) {
            s.player_name = Some(character.name.clone());
            s.permission = permission;
        }
        let _ = output_tx.send(SessionOutput::new(
            session_id,
            format!(
                "재접속 완료! 환영합니다, {}님!",
                character.name
            ),
        ));
        tracing::info!(character_id = character.id, ?entity, "Player reconnected to lingering entity");
        return Some(PlayerInput {
            session_id,
            entity,
            action: PlayerAction::Look,
        });
    }

    // No lingering entity — spawn fresh from DB
    let entity = ecs.spawn_entity();

    // Set name
    ecs.set_component(entity, Name(character.name.clone())).unwrap();
    ecs.set_component(entity, PlayerTag).unwrap();

    // Restore components from DB JSON
    if let Some(hp) = character.components.get("Health") {
        if let (Some(current), Some(max)) = (hp.get("current"), hp.get("max")) {
            ecs.set_component(
                entity,
                Health {
                    current: current.as_i64().unwrap_or(100) as i32,
                    max: max.as_i64().unwrap_or(100) as i32,
                },
            )
            .unwrap();
        }
    } else {
        ecs.set_component(entity, Health { current: 100, max: 100 }).unwrap();
    }

    if let Some(atk) = character.components.get("Attack") {
        ecs.set_component(entity, Attack(atk.as_i64().unwrap_or(10) as i32)).unwrap();
    } else {
        ecs.set_component(entity, Attack(10)).unwrap();
    }

    if let Some(def) = character.components.get("Defense") {
        ecs.set_component(entity, Defense(def.as_i64().unwrap_or(3) as i32)).unwrap();
    } else {
        ecs.set_component(entity, Defense(3)).unwrap();
    }

    ecs.set_component(entity, Inventory::new()).unwrap();

    // Place in room (restore from DB or use spawn room)
    let target_room = character
        .room_id
        .map(ecs_adapter::EntityId::from_u64)
        .filter(|&rid| space.room_exists(rid))
        .unwrap_or(spawn_room);
    space.place_entity(entity, target_room).unwrap();

    sessions.bind_entity(session_id, entity);
    if let Some(s) = sessions.get_session_mut(session_id) {
        s.player_name = Some(character.name.clone());
        s.character_id = Some(character.id);
        s.permission = permission;
    }

    let _ = output_tx.send(SessionOutput::new(
        session_id,
        format!(
            "환영합니다, {}님!\n'도움말'을 입력하면 명령어를 볼 수 있습니다.",
            character.name
        ),
    ));

    Some(PlayerInput {
        session_id,
        entity,
        action: PlayerAction::Look,
    })
}

fn spawn_player_entity(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    output_tx: &OutputTx,
    session_id: SessionId,
    name: &str,
    spawn_room: ecs_adapter::EntityId,
) -> Option<PlayerInput> {
    let entity = ecs.spawn_entity();
    ecs.set_component(entity, Name(name.to_string())).unwrap();
    ecs.set_component(entity, PlayerTag).unwrap();
    ecs.set_component(entity, Health { current: 100, max: 100 }).unwrap();
    ecs.set_component(entity, Attack(10)).unwrap();
    ecs.set_component(entity, Defense(3)).unwrap();
    ecs.set_component(entity, Inventory::new()).unwrap();
    space.place_entity(entity, spawn_room).unwrap();

    sessions.bind_entity(session_id, entity);
    if let Some(s) = sessions.get_session_mut(session_id) {
        s.player_name = Some(name.to_string());
    }

    let _ = output_tx.send(SessionOutput::new(
        session_id,
        format!("환영합니다, {}님!\n'도움말'을 입력하면 명령어를 볼 수 있습니다.", name),
    ));

    Some(PlayerInput {
        session_id,
        entity,
        action: PlayerAction::Look,
    })
}

fn handle_disconnect(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    session_id: SessionId,
    player_db: Option<&PlayerDb>,
    current_tick: u64,
) {
    // Save character state before disconnect
    if let Some(session) = sessions.get_session(session_id) {
        if let (Some(entity), Some(character_id), Some(account_id)) =
            (session.entity, session.character_id, session.account_id)
        {
            // Save to DB
            if let Some(db) = player_db {
                save_character_state(ecs, space, entity, character_id, db);
            }

            // Auth mode: linger instead of immediate despawn
            if player_db.is_some() {
                sessions.disconnect(session_id);
                sessions.add_lingering(session::LingeringEntity {
                    entity,
                    character_id,
                    account_id,
                    disconnect_tick: current_tick,
                });
                sessions.remove_session(session_id);
                tracing::info!(character_id, ?entity, "Player disconnected, entity lingering");
                return;
            }
        }
    }

    // Quick-play mode: immediate despawn
    if let Some(entity) = sessions.disconnect(session_id) {
        let _ = space.remove_entity(entity);
        let _ = ecs.despawn_entity(entity);
    }
    sessions.remove_session(session_id);
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
