/// Integration test: WebSocket connection -> Grid mode networking with AOI + Delta Snapshot.
use std::collections::BTreeMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use ecs_adapter::EntityId;
use engine_core::tick::{TickConfig, TickLoop};
use project_2d::components::Name;
use net::channels::{NetToTick, OutputTx, PlayerRx};
use net::protocol::{EntityMovedWire, EntityWire, GridConfigWire, ServerMessage};
use session::{SessionId, SessionManager, SessionOutput, SessionState};
use space::grid_space::{GridConfig, GridPos};
use space::{GridSpace, SpaceModel};

const AOI_RADIUS: u32 = 32;

/// Per-session AOI tracking state (mirrors main.rs AoiTracker).
struct TestAoiState {
    known: BTreeMap<EntityId, GridPos>,
}

struct TestAoiTracker {
    sessions: BTreeMap<SessionId, TestAoiState>,
    radius: u32,
}

impl TestAoiTracker {
    fn new(radius: u32) -> Self {
        Self {
            sessions: BTreeMap::new(),
            radius,
        }
    }

    fn on_session_playing(&mut self, session_id: SessionId) {
        self.sessions.insert(
            session_id,
            TestAoiState {
                known: BTreeMap::new(),
            },
        );
    }

    fn on_session_removed(&mut self, session_id: SessionId) {
        self.sessions.remove(&session_id);
    }
}

/// Run one grid tick: process network messages, step engine, broadcast AOI delta.
fn run_grid_tick(
    tick_loop: &mut TickLoop<GridSpace>,
    sessions: &mut SessionManager,
    player_rx: &mut PlayerRx,
    output_tx: &OutputTx,
    grid_config: &GridConfig,
    aoi: &mut TestAoiTracker,
) {
    // Process network messages
    while let Ok(msg) = player_rx.try_recv() {
        match msg {
            NetToTick::NewConnection { session_id } => {
                sessions.create_session_with_id(session_id);
            }
            NetToTick::PlayerInput { session_id, line } => {
                let state = match sessions.get_session(session_id) {
                    Some(s) => s.state.clone(),
                    None => continue,
                };
                match state {
                    SessionState::AwaitingLogin => {
                        let name = line.trim().to_string();
                        if name.is_empty() {
                            continue;
                        }
                        let entity = tick_loop.ecs.spawn_entity();
                        let cx = grid_config.origin_x + (grid_config.width as i32) / 2;
                        let cy = grid_config.origin_y + (grid_config.height as i32) / 2;
                        tick_loop
                            .ecs
                            .set_component(entity, Name(name.clone()))
                            .unwrap();
                        tick_loop.space.set_position(entity, cx, cy).unwrap();
                        sessions.bind_entity(session_id, entity);
                        if let Some(s) = sessions.get_session_mut(session_id) {
                            s.player_name = Some(name);
                        }
                        aoi.on_session_playing(session_id);
                        let welcome = ServerMessage::Welcome {
                            session_id: session_id.0,
                            entity_id: entity.to_u64(),
                            tick: tick_loop.current_tick,
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
                    }
                    SessionState::Playing => {
                        let entity = match sessions.get_session(session_id).and_then(|s| s.entity)
                        {
                            Some(e) => e,
                            None => continue,
                        };
                        if line == "__ping" {
                            let pong = ServerMessage::Pong;
                            let _ = output_tx.send(SessionOutput::new(
                                session_id,
                                serde_json::to_string(&pong).unwrap(),
                            ));
                            continue;
                        }
                        if let Some(rest) = line.strip_prefix("__grid_move ") {
                            let parts: Vec<&str> = rest.split_whitespace().collect();
                            if parts.len() == 2 {
                                if let (Ok(dx), Ok(dy)) =
                                    (parts[0].parse::<i32>(), parts[1].parse::<i32>())
                                {
                                    if let Some(pos) = tick_loop.space.get_position(entity) {
                                        let _ = tick_loop
                                            .space
                                            .move_to(entity, pos.x + dx, pos.y + dy);
                                    }
                                }
                            }
                        }
                    }
                    SessionState::Disconnected => {}
                    _ => {} // AwaitingPassword, etc. not used in grid tests
                }
            }
            NetToTick::Disconnected { session_id } => {
                if let Some(entity) = sessions.disconnect(session_id) {
                    let _ = tick_loop.space.remove_entity(entity);
                    let _ = tick_loop.ecs.despawn_entity(entity);
                }
                aoi.on_session_removed(session_id);
                sessions.remove_session(session_id);
            }
        }
    }

    let _metrics = tick_loop.step();

    // Broadcast AOI delta
    let playing = sessions.playing_sessions();
    if playing.is_empty() {
        return;
    }

    let all_positions = tick_loop.space.all_entity_positions();
    let mut name_cache: BTreeMap<EntityId, Option<String>> = BTreeMap::new();

    for session in &playing {
        let self_entity = match session.entity {
            Some(e) => e,
            None => continue,
        };
        let player_pos = match tick_loop.space.get_position(self_entity) {
            Some(p) => p,
            None => continue,
        };

        let aoi_state = match aoi.sessions.get_mut(&session.session_id) {
            Some(s) => s,
            None => continue,
        };

        let in_radius =
            tick_loop
                .space
                .entities_in_radius(player_pos.x, player_pos.y, aoi.radius);
        let current_aoi: BTreeMap<EntityId, GridPos> = in_radius
            .into_iter()
            .filter_map(|eid| all_positions.get(&eid).map(|pos| (eid, *pos)))
            .collect();

        let mut entered = Vec::new();
        let mut moved = Vec::new();
        let mut left = Vec::new();

        for (eid, _) in aoi_state.known.iter() {
            if !current_aoi.contains_key(eid) {
                left.push(eid.to_u64());
            }
        }

        for (&eid, &pos) in &current_aoi {
            match aoi_state.known.get(&eid) {
                None => {
                    let name = name_cache
                        .entry(eid)
                        .or_insert_with(|| {
                            tick_loop
                                .ecs
                                .get_component::<Name>(eid)
                                .ok()
                                .map(|n| n.0.clone())
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
                        moved.push(EntityMovedWire {
                            id: eid.to_u64(),
                            x: pos.x,
                            y: pos.y,
                        });
                    }
                }
            }
        }

        aoi_state.known = current_aoi;

        let delta = ServerMessage::StateDelta {
            tick: tick_loop.current_tick,
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

#[tokio::test]
async fn ws_connect_and_welcome() {
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (register_tx, register_rx) = mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

    // Output router
    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    // WS server on random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    tokio::spawn(net::ws_server::run_ws_server(
        addr.to_string(),
        player_tx,
        register_tx,
        unregister_tx,
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Grid tick loop setup
    let grid_config = GridConfig {
        width: 100,
        height: 100,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    // Connect WS client
    let url = format!("ws://{}", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process connection
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Send connect message
    let connect_msg = serde_json::to_string(&serde_json::json!({"type":"connect","name":"Alice"})).unwrap();
    ws.send(Message::Text(connect_msg.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process login
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Read Welcome message
    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let server_msg: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(server_msg["type"], "welcome");
    assert!(server_msg["session_id"].as_u64().unwrap() >= 1_000_000);
    assert!(server_msg["entity_id"].is_u64());
    assert_eq!(server_msg["grid_config"]["width"], 100);
    assert_eq!(server_msg["grid_config"]["height"], 100);

    // Should receive state_delta with self entity entered
    let msg2 = ws.next().await.unwrap().unwrap();
    let text2 = msg2.into_text().unwrap();
    let delta: serde_json::Value = serde_json::from_str(&text2).unwrap();
    assert_eq!(delta["type"], "state_delta");
    let entered = delta["entered"].as_array().unwrap();
    assert_eq!(entered.len(), 1);
    assert_eq!(entered[0]["is_self"], true);
    assert_eq!(entered[0]["x"], 50); // center of 100-wide grid
    assert_eq!(entered[0]["y"], 50);

    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn ws_move_changes_position() {
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (register_tx, register_rx) = mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    tokio::spawn(net::ws_server::run_ws_server(
        addr.to_string(),
        player_tx,
        register_tx,
        unregister_tx,
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;

    let grid_config = GridConfig {
        width: 100,
        height: 100,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    let url = format!("ws://{}", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process new connection
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Connect
    let connect = serde_json::to_string(&serde_json::json!({"type":"connect","name":"Bob"})).unwrap();
    ws.send(Message::Text(connect.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drain welcome + state_delta (entered)
    let _welcome = ws.next().await.unwrap().unwrap();
    let _delta1 = ws.next().await.unwrap().unwrap();

    // Send move command (dx=1, dy=0)
    let move_msg = serde_json::to_string(&serde_json::json!({"type":"move","dx":1,"dy":0})).unwrap();
    ws.send(Message::Text(move_msg.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Read state_delta with moved
    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let delta: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(delta["type"], "state_delta");
    let moved = delta["moved"].as_array().unwrap();
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0]["x"], 51); // moved from 50 to 51
    assert_eq!(moved[0]["y"], 50);

    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn ws_disconnect_removes_entity() {
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (register_tx, register_rx) = mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    tokio::spawn(net::ws_server::run_ws_server(
        addr.to_string(),
        player_tx,
        register_tx,
        unregister_tx,
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;

    let grid_config = GridConfig {
        width: 100,
        height: 100,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    // Connect player 1
    let url = format!("ws://{}", addr);
    let (mut ws1, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    ws1.send(Message::Text(serde_json::to_string(&serde_json::json!({"type":"connect","name":"P1"})).unwrap().into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drain P1's welcome + state_delta
    let _w1 = ws1.next().await.unwrap().unwrap();
    let _d1 = ws1.next().await.unwrap().unwrap();

    // Connect player 2
    let (mut ws2, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process P2's NewConnection
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Drain P1's state_delta from tick that only processed P2's NewConnection (no entity yet, empty delta)
    let p1_interim = ws1.next().await.unwrap().unwrap();
    let p1_interim_text = p1_interim.into_text().unwrap();
    let p1_interim_val: serde_json::Value = serde_json::from_str(&p1_interim_text).unwrap();
    assert_eq!(p1_interim_val["type"], "state_delta");

    // Now send P2's connect (login) message
    ws2.send(Message::Text(serde_json::to_string(&serde_json::json!({"type":"connect","name":"P2"})).unwrap().into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drain P2's welcome + state_delta
    let _w2 = ws2.next().await.unwrap().unwrap();
    let _d2 = ws2.next().await.unwrap().unwrap();
    // P1 receives state_delta with P2 as entered
    let p1_delta_msg = ws1.next().await.unwrap().unwrap();
    let p1_text = p1_delta_msg.into_text().unwrap();
    let p1_delta: serde_json::Value = serde_json::from_str(&p1_text).unwrap();
    assert_eq!(p1_delta["type"], "state_delta");
    let p1_entered = p1_delta["entered"].as_array().unwrap();
    assert_eq!(p1_entered.len(), 1); // P2 entered P1's AOI

    // Disconnect P2
    ws2.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // P1 should receive state_delta with P2 in left
    let msg = ws1.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["type"], "state_delta");
    let left = val["left"].as_array().unwrap();
    assert_eq!(left.len(), 1); // P2 left

    ws1.close(None).await.unwrap();
}

#[tokio::test]
async fn ws_ping_pong() {
    let (player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (register_tx, register_rx) = mpsc::unbounded_channel();
    let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

    tokio::spawn(net::output_router::run_output_router(
        output_rx,
        register_rx,
        unregister_rx,
    ));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    tokio::spawn(net::ws_server::run_ws_server(
        addr.to_string(),
        player_tx,
        register_tx,
        unregister_tx,
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;

    let grid_config = GridConfig {
        width: 100,
        height: 100,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    let url = format!("ws://{}", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Process new connection + login
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(50)).await;

    ws.send(Message::Text(serde_json::to_string(&serde_json::json!({"type":"connect","name":"PingPlayer"})).unwrap().into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Drain welcome + state_delta
    let _welcome = ws.next().await.unwrap().unwrap();
    let _delta = ws.next().await.unwrap().unwrap();

    // Send ping
    ws.send(Message::Text(r#"{"type":"ping"}"#.to_string().into()))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should receive pong before state_delta
    let msg = ws.next().await.unwrap().unwrap();
    let text = msg.into_text().unwrap();
    let val: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(val["type"], "pong");

    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn ws_aoi_filters_distant_entity() {
    // Test: an entity outside AOI_RADIUS should NOT appear in state_delta
    let (_player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, mut output_rx) = mpsc::unbounded_channel();

    let grid_config = GridConfig {
        width: 256,
        height: 256,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    // Create player at (128, 128)
    let session_id = SessionId(1_000_001);
    sessions.create_session_with_id(session_id);
    let player_entity = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(player_entity, Name("Player".to_string()))
        .unwrap();
    tick_loop.space.set_position(player_entity, 128, 128).unwrap();
    sessions.bind_entity(session_id, player_entity);
    aoi.on_session_playing(session_id);

    // Create distant entity at (200, 200) — Chebyshev distance = 72, beyond AOI_RADIUS=32
    let distant_entity = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(distant_entity, Name("FarAway".to_string()))
        .unwrap();
    tick_loop.space.set_position(distant_entity, 200, 200).unwrap();

    // Create nearby entity at (130, 130) — Chebyshev distance = 2, within AOI_RADIUS
    let near_entity = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(near_entity, Name("NearBy".to_string()))
        .unwrap();
    tick_loop.space.set_position(near_entity, 130, 130).unwrap();

    // Run a tick to generate delta
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );

    // Collect output
    let mut deltas = Vec::new();
    while let Ok(out) = output_rx.try_recv() {
        deltas.push(out);
    }

    // Should have exactly 1 message for the player
    assert_eq!(deltas.len(), 1);
    let delta: serde_json::Value = serde_json::from_str(&deltas[0].text).unwrap();
    assert_eq!(delta["type"], "state_delta");

    let entered = delta["entered"].as_array().unwrap();
    // Should contain self (player) and nearby entity, but NOT distant entity
    assert_eq!(entered.len(), 2);

    let entered_ids: Vec<u64> = entered.iter().map(|e| e["id"].as_u64().unwrap()).collect();
    assert!(entered_ids.contains(&player_entity.to_u64()));
    assert!(entered_ids.contains(&near_entity.to_u64()));
    assert!(!entered_ids.contains(&distant_entity.to_u64()));
}

#[tokio::test]
async fn ws_aoi_enter_leave_on_move() {
    // Test: when player moves, entities enter/leave AOI correctly
    let (_player_tx, mut player_rx) = mpsc::unbounded_channel();
    let (output_tx, mut output_rx) = mpsc::unbounded_channel();

    let grid_config = GridConfig {
        width: 256,
        height: 256,
        origin_x: 0,
        origin_y: 0,
    };
    let config = TickConfig {
        tps: 10,
        max_ticks: 0,
    };
    let mut tick_loop = TickLoop::new(config, GridSpace::new(grid_config.clone()));
    let mut sessions = SessionManager::new();
    let mut aoi = TestAoiTracker::new(AOI_RADIUS);

    // Player at (50, 128)
    let session_id = SessionId(1_000_002);
    sessions.create_session_with_id(session_id);
    let player_entity = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(player_entity, Name("Mover".to_string()))
        .unwrap();
    tick_loop.space.set_position(player_entity, 50, 128).unwrap();
    sessions.bind_entity(session_id, player_entity);
    aoi.on_session_playing(session_id);

    // Entity A at (80, 128) — Chebyshev distance 30 from player, within AOI
    let entity_a = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(entity_a, Name("EntityA".to_string()))
        .unwrap();
    tick_loop.space.set_position(entity_a, 80, 128).unwrap();

    // Entity B at (120, 128) — Chebyshev distance 70 from player, outside AOI
    let entity_b = tick_loop.ecs.spawn_entity();
    tick_loop
        .ecs
        .set_component(entity_b, Name("EntityB".to_string()))
        .unwrap();
    tick_loop.space.set_position(entity_b, 120, 128).unwrap();

    // Tick 1: initial state — player sees self + entity_a
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );

    let out1 = output_rx.try_recv().unwrap();
    let delta1: serde_json::Value = serde_json::from_str(&out1.text).unwrap();
    let entered1 = delta1["entered"].as_array().unwrap();
    assert_eq!(entered1.len(), 2); // self + entity_a
    let entered1_ids: Vec<u64> = entered1.iter().map(|e| e["id"].as_u64().unwrap()).collect();
    assert!(entered1_ids.contains(&player_entity.to_u64()));
    assert!(entered1_ids.contains(&entity_a.to_u64()));
    assert!(!entered1_ids.contains(&entity_b.to_u64()));

    // Teleport player to (90, 128) — now entity_a is at distance 10 (still in), entity_b at distance 30 (now in AOI!)
    tick_loop.space.set_position(player_entity, 90, 128).unwrap();

    // Tick 2: entity_b should enter AOI
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );

    let out2 = output_rx.try_recv().unwrap();
    let delta2: serde_json::Value = serde_json::from_str(&out2.text).unwrap();
    assert_eq!(delta2["type"], "state_delta");
    // Player moved, so player should be in "moved" list
    let moved2 = delta2["moved"].as_array().unwrap();
    assert!(moved2.iter().any(|m| m["id"].as_u64().unwrap() == player_entity.to_u64()));
    // Entity B should now be "entered"
    let entered2 = delta2["entered"].as_array().unwrap();
    assert!(entered2.iter().any(|e| e["id"].as_u64().unwrap() == entity_b.to_u64()));

    // Teleport player far away to (200, 128) — entity_a (at 80) is distance 120, entity_b (at 120) is distance 80
    // Both should leave AOI
    tick_loop.space.set_position(player_entity, 200, 128).unwrap();

    // Tick 3: entity_a and entity_b should leave
    run_grid_tick(
        &mut tick_loop,
        &mut sessions,
        &mut player_rx,
        &output_tx,
        &grid_config,
        &mut aoi,
    );

    let out3 = output_rx.try_recv().unwrap();
    let delta3: serde_json::Value = serde_json::from_str(&out3.text).unwrap();
    assert_eq!(delta3["type"], "state_delta");
    let left3 = delta3["left"].as_array().unwrap();
    let left3_ids: Vec<u64> = left3.iter().map(|l| l.as_u64().unwrap()).collect();
    assert!(left3_ids.contains(&entity_a.to_u64()));
    assert!(left3_ids.contains(&entity_b.to_u64()));
    // Player's own movement should be in "moved"
    let moved3 = delta3["moved"].as_array().unwrap();
    assert!(moved3.iter().any(|m| m["id"].as_u64().unwrap() == player_entity.to_u64()));
}
