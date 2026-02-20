/// Integration test: TCP connection -> Login -> Move (actual network I/O).
use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use engine_core::tick::{TickConfig, TickLoop};
use mud::components::*;
use mud::output::SessionOutput;
use mud::parser::{parse_input, PlayerAction};
use mud::script_setup::register_mud_script_components;
use mud::session::{SessionManager, SessionState};
use mud::systems::{GameContext, PlayerInput};
use net::channels::{NetToTick, OutputTx, PlayerRx};
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ScriptConfig;
use space::SpaceModel;

fn scripts_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts"))
}

/// A simplified tick runner for testing (runs N ticks then stops).
fn run_test_ticks(
    tick_loop: &mut TickLoop<space::RoomGraphSpace>,
    sessions: &mut SessionManager,
    player_rx: &mut PlayerRx,
    output_tx: &OutputTx,
    spawn_room: ecs_adapter::EntityId,
    script_engine: &ScriptEngine,
    num_ticks: usize,
) {
    for _ in 0..num_ticks {
        let mut inputs = Vec::new();
        while let Ok(msg) = player_rx.try_recv() {
            match msg {
                NetToTick::NewConnection { session_id } => {
                    sessions.create_session_with_id(session_id);
                    let _ = output_tx.send(SessionOutput::new(
                        session_id,
                        "Rust MUD에 오신 것을 환영합니다!\n이름을 입력하세요:",
                    ));
                    // Fire on_connect hooks
                    let mut script_ctx = scripting::engine::ScriptContext {
                        ecs: &mut tick_loop.ecs,
                        space: &mut tick_loop.space,
                        sessions,
                        tick: tick_loop.current_tick,
                    };
                    if let Ok(connect_outputs) = script_engine.run_on_connect(&mut script_ctx, session_id) {
                        for out in connect_outputs {
                            let _ = output_tx.send(out);
                        }
                    }
                }
                NetToTick::PlayerInput { session_id, line } => {
                    let session = sessions.get_session(session_id);
                    if let Some(session) = session {
                        match session.state {
                            SessionState::AwaitingLogin => {
                                let name = line.trim().to_string();
                                if !name.is_empty() {
                                    let entity = tick_loop.ecs.spawn_entity();
                                    tick_loop.ecs.set_component(entity, Name(name.clone())).unwrap();
                                    tick_loop.ecs.set_component(entity, PlayerTag).unwrap();
                                    tick_loop.ecs.set_component(entity, Health { current: 100, max: 100 }).unwrap();
                                    tick_loop.ecs.set_component(entity, Attack(10)).unwrap();
                                    tick_loop.ecs.set_component(entity, Defense(3)).unwrap();
                                    tick_loop.ecs.set_component(entity, Inventory::new()).unwrap();
                                    tick_loop.space.place_entity(entity, spawn_room).unwrap();
                                    sessions.bind_entity(session_id, entity);
                                    if let Some(s) = sessions.get_session_mut(session_id) {
                                        s.player_name = Some(name.clone());
                                    }
                                    let _ = output_tx.send(SessionOutput::new(
                                        session_id,
                                        format!("환영합니다, {}님!", name),
                                    ));
                                    // Queue a look action
                                    inputs.push(PlayerInput {
                                        session_id,
                                        entity,
                                        action: PlayerAction::Look,
                                    });
                                }
                            }
                            SessionState::Playing => {
                                let entity = session.entity.unwrap();
                                let action = parse_input(&line);
                                if action != PlayerAction::Quit {
                                    inputs.push(PlayerInput {
                                        session_id,
                                        entity,
                                        action,
                                    });
                                }
                            }
                            SessionState::Disconnected => {}
                        }
                    }
                }
                NetToTick::Disconnected { session_id } => {
                    if let Some(entity) = sessions.disconnect(session_id) {
                        let _ = tick_loop.space.remove_entity(entity);
                        let _ = tick_loop.ecs.despawn_entity(entity);
                    }
                    sessions.remove_session(session_id);
                }
            }
        }

        let _metrics = tick_loop.step();

        let mut ctx = GameContext {
            ecs: &mut tick_loop.ecs,
            space: &mut tick_loop.space,
            sessions,
            tick: tick_loop.current_tick,
        };
        let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(script_engine));
        for output in outputs {
            let _ = output_tx.send(output);
        }

        // Run on_tick for combat etc.
        {
            let mut script_ctx = ScriptContext {
                ecs: &mut tick_loop.ecs,
                space: &mut tick_loop.space,
                sessions,
                tick: tick_loop.current_tick,
            };
            if let Ok(tick_outputs) = script_engine.run_on_tick(&mut script_ctx) {
                for output in tick_outputs {
                    let _ = output_tx.send(output);
                }
            }
        }
    }
}

#[tokio::test]
async fn tcp_login_and_move() {
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (register_tx, register_rx) = mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

    // Start output router
    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    // Start TCP server on random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    tokio::spawn(net::server::run_tcp_server(
        addr.to_string(),
        player_tx,
        register_tx,
        unregister_tx,
    ));

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Setup game world via scripts
    let config = TickConfig { tps: 10, max_ticks: 0 };
    let mut tick_loop = TickLoop::new(config, space::RoomGraphSpace::new());
    let mut sessions = SessionManager::new();

    let mut script_engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    register_mud_script_components(script_engine.component_registry_mut());
    script_engine.load_directory(scripts_dir()).unwrap();

    // Run on_init to create world
    {
        let mut ctx = ScriptContext {
            ecs: &mut tick_loop.ecs,
            space: &mut tick_loop.space,
            sessions: &sessions,
            tick: 0,
        };
        script_engine.run_on_init(&mut ctx).unwrap();
    }

    // Find spawn room
    let spawn_room = tick_loop
        .ecs
        .entities_with::<Name>()
        .into_iter()
        .find(|&eid| {
            tick_loop
                .ecs
                .get_component::<Name>(eid)
                .map(|n| n.0 == "시작의 방")
                .unwrap_or(false)
        })
        .expect("시작의 방 not found");

    // Connect client
    let mut stream = TcpStream::connect(addr).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process connection
    run_test_ticks(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        spawn_room,
        &script_engine,
        2,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Read welcome message
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf).await.unwrap();
    let welcome = String::from_utf8_lossy(&buf[..n]);
    assert!(welcome.contains("Rust MUD에 오신 것을 환영합니다"), "Got: {}", welcome);

    // Send name
    stream.write_all(b"TestHero\n").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_test_ticks(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        spawn_room,
        &script_engine,
        2,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    let n = stream.read(&mut buf).await.unwrap();
    let login_msg = String::from_utf8_lossy(&buf[..n]);
    assert!(login_msg.contains("환영합니다, TestHero"), "Got: {}", login_msg);
    assert!(login_msg.contains("시작의 방"), "Got: {}", login_msg);

    // Move east (using Korean command)
    stream.write_all("동\n".as_bytes()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_test_ticks(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        spawn_room,
        &script_engine,
        2,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    let n = stream.read(&mut buf).await.unwrap();
    let move_msg = String::from_utf8_lossy(&buf[..n]);
    assert!(move_msg.contains("시장 광장"), "Got: {}", move_msg);

    drop(stream);
}
