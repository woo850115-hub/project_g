/// Integration test: Login -> Move -> Combat -> Inventory full flow (no network).
/// Game logic now runs via Lua scripts loaded from scripts/ directory.
use std::path::Path;

use ecs_adapter::{EcsAdapter, EntityId};
use mud::components::*;
use mud::output::SessionId;
use mud::parser::{Direction, PlayerAction};
use mud::script_setup::register_mud_script_components;
use mud::session::SessionManager;
use mud::systems::{GameContext, PlayerInput};
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ScriptConfig;
use space::{RoomGraphSpace, SpaceModel};

fn scripts_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts"))
}

fn setup() -> (EcsAdapter, RoomGraphSpace, SessionManager, ScriptEngine) {
    let mut ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    let sessions = SessionManager::new();

    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    register_mud_script_components(engine.component_registry_mut());
    engine.load_directory(scripts_dir()).unwrap();

    // Run on_init to create the world
    let mut ctx = ScriptContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    engine.run_on_init(&mut ctx).unwrap();

    (ecs, space, sessions, engine)
}

/// Find an entity by Name component.
fn find_entity_by_name(ecs: &EcsAdapter, name: &str) -> Option<EntityId> {
    ecs.entities_with::<Name>()
        .into_iter()
        .find(|&eid| {
            ecs.get_component::<Name>(eid)
                .map(|n| n.0 == name)
                .unwrap_or(false)
        })
}

fn spawn_room(ecs: &EcsAdapter) -> EntityId {
    find_entity_by_name(ecs, "시작의 방").expect("시작의 방 not found")
}

fn spawn_player(
    ecs: &mut EcsAdapter,
    space: &mut RoomGraphSpace,
    sessions: &mut SessionManager,
    name: &str,
    room: EntityId,
) -> (SessionId, EntityId) {
    let sid = sessions.create_session();
    let entity = ecs.spawn_entity();
    ecs.set_component(entity, Name(name.to_string())).unwrap();
    ecs.set_component(entity, PlayerTag).unwrap();
    ecs.set_component(entity, Health { current: 100, max: 100 }).unwrap();
    ecs.set_component(entity, Attack(10)).unwrap();
    ecs.set_component(entity, Defense(3)).unwrap();
    ecs.set_component(entity, Inventory::new()).unwrap();
    space.place_entity(entity, room).unwrap();
    sessions.bind_entity(sid, entity);
    if let Some(s) = sessions.get_session_mut(sid) {
        s.player_name = Some(name.to_string());
    }
    (sid, entity)
}

#[test]
fn look_shows_room_description() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", room);

    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Look,
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(!outputs.is_empty());
    let text = &outputs[0].text;
    assert!(text.contains("시작의 방"), "Expected room name, got: {}", text);
    assert!(text.contains("환영합니다, 모험가여"), "Expected description, got: {}", text);
}

#[test]
fn move_east_to_market_square() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", room);

    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Move(Direction::East),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(!outputs.is_empty());
    let player_output: Vec<_> = outputs.iter().filter(|o| o.session_id == sid).collect();
    assert!(!player_output.is_empty());
    let text = &player_output.last().unwrap().text;
    assert!(text.contains("시장 광장"), "Expected market square, got: {}", text);

    // Player should now be in market square
    let market = find_entity_by_name(&ecs, "시장 광장").unwrap();
    assert_eq!(space.entity_room(entity), Some(market));
}

#[test]
fn move_to_invalid_direction_fails() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", room);

    // Starting room only has east exit
    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Move(Direction::North),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(!outputs.is_empty());
    assert!(outputs[0].text.contains("출구가 없습니다"), "Got: {}", outputs[0].text);

    // Player should still be in spawn room
    assert_eq!(space.entity_room(entity), Some(room));
}

#[test]
fn full_combat_flow() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let dungeon = find_entity_by_name(&ecs, "던전 1층").unwrap();
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", dungeon);
    let goblin = find_entity_by_name(&ecs, "고블린").unwrap();

    // Attack goblin (on_action)
    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Attack("고블린".to_string()),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 1,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    // Should see attack message
    let attack_msg = outputs.iter().find(|o| o.session_id == sid && o.text.contains("공격"));
    assert!(attack_msg.is_some(), "Should see attack message: {:?}", outputs);

    // Run on_tick to resolve combat (first round)
    {
        let mut script_ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: 1,
        };
        let tick_outputs = engine.run_on_tick(&mut script_ctx).unwrap();
        let hit_msg = tick_outputs.iter().find(|o| o.session_id == sid && o.text.contains("데미지"));
        assert!(hit_msg.is_some(), "Should see hit message: {:?}", tick_outputs);
    }

    // Run combat for multiple ticks until goblin dies
    let goblin_max_hp = 30;
    let player_atk = 10;
    let goblin_def = 2;
    let damage_per_tick = (player_atk - goblin_def).max(1);
    let ticks_to_kill = (goblin_max_hp + damage_per_tick - 1) / damage_per_tick;

    for tick in 2..=(ticks_to_kill + 5) {
        // Re-attack if combat target was cleared and goblin not dead
        if !ecs.has_component::<CombatTarget>(entity)
            && !ecs.has_component::<Dead>(goblin)
        {
            let inputs = vec![PlayerInput {
                session_id: sid,
                entity,
                action: PlayerAction::Attack("고블린".to_string()),
            }];
            let mut ctx = GameContext {
                ecs: &mut ecs,
                space: &mut space,
                sessions: &sessions,
                tick: tick as u64,
            };
            mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));
        }

        // Run on_tick for combat resolution
        let mut script_ctx = ScriptContext {
            ecs: &mut ecs,
            space: &mut space,
            sessions: &sessions,
            tick: tick as u64,
        };
        engine.run_on_tick(&mut script_ctx).unwrap();
    }

    // Goblin should be dead
    assert!(ecs.has_component::<Dead>(goblin), "Goblin should be dead");
}

#[test]
fn inventory_get_and_drop() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let market = find_entity_by_name(&ecs, "시장 광장").unwrap();
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", market);
    let potion = find_entity_by_name(&ecs, "치유 물약").unwrap();

    // Get potion
    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Get("물약".to_string()),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(outputs.iter().any(|o| o.text.contains("주웠습니다")), "Get output: {:?}", outputs);

    // Check inventory
    let inv = ecs.get_component::<Inventory>(entity).unwrap();
    assert_eq!(inv.items.len(), 1);
    assert_eq!(inv.items[0], potion);

    // Potion should no longer be in the room
    assert_eq!(space.entity_room(potion), None);

    // List inventory
    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::InventoryList,
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 1,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));
    assert!(outputs.iter().any(|o| o.text.contains("치유 물약")), "Inventory output: {:?}", outputs);

    // Drop potion
    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Drop("물약".to_string()),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 2,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));
    assert!(outputs.iter().any(|o| o.text.contains("버렸습니다")), "Drop output: {:?}", outputs);

    // Potion should be back in the room
    assert_eq!(space.entity_room(potion), Some(market));

    // Inventory should be empty
    let inv = ecs.get_component::<Inventory>(entity).unwrap();
    assert!(inv.items.is_empty());
}

#[test]
fn who_command_shows_players() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let market = find_entity_by_name(&ecs, "시장 광장").unwrap();
    let (sid1, entity1) = spawn_player(&mut ecs, &mut space, &mut sessions, "Alice", room);
    let (_sid2, _entity2) = spawn_player(&mut ecs, &mut space, &mut sessions, "Bob", market);

    let inputs = vec![PlayerInput {
        session_id: sid1,
        entity: entity1,
        action: PlayerAction::Who,
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(!outputs.is_empty());
    let text = &outputs[0].text;
    assert!(text.contains("Alice"), "Who output: {}", text);
    assert!(text.contains("Bob"), "Who output: {}", text);
    assert!(text.contains("2"), "Who output: {}", text);
}

#[test]
fn say_broadcasts_to_room() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid1, entity1) = spawn_player(&mut ecs, &mut space, &mut sessions, "Alice", room);
    let (sid2, _entity2) = spawn_player(&mut ecs, &mut space, &mut sessions, "Bob", room);

    let inputs = vec![PlayerInput {
        session_id: sid1,
        entity: entity1,
        action: PlayerAction::Say("hello everyone".to_string()),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    // Alice sees "당신이 말합니다: hello everyone"
    let alice_msg = outputs.iter().find(|o| o.session_id == sid1);
    assert!(alice_msg.unwrap().text.contains("당신이 말합니다"), "Alice output: {:?}", outputs);

    // Bob sees "Alice님이 말합니다: hello everyone"
    let bob_msg = outputs.iter().find(|o| o.session_id == sid2);
    assert!(bob_msg.is_some(), "Bob should receive message");
    assert!(bob_msg.unwrap().text.contains("Alice님이 말합니다"), "Bob output: {:?}", outputs);
}

#[test]
fn help_command() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid, entity) = spawn_player(&mut ecs, &mut space, &mut sessions, "Hero", room);

    let inputs = vec![PlayerInput {
        session_id: sid,
        entity,
        action: PlayerAction::Help,
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    assert!(!outputs.is_empty());
    assert!(outputs[0].text.contains("사용 가능한 명령어"), "Help output: {}", outputs[0].text);
}

#[test]
fn move_broadcasts_to_others() {
    let (mut ecs, mut space, mut sessions, engine) = setup();
    let room = spawn_room(&ecs);
    let (sid1, entity1) = spawn_player(&mut ecs, &mut space, &mut sessions, "Alice", room);
    let (sid2, _entity2) = spawn_player(&mut ecs, &mut space, &mut sessions, "Bob", room);

    // Alice moves east
    let inputs = vec![PlayerInput {
        session_id: sid1,
        entity: entity1,
        action: PlayerAction::Move(Direction::East),
    }];
    let mut ctx = GameContext {
        ecs: &mut ecs,
        space: &mut space,
        sessions: &sessions,
        tick: 0,
    };
    let outputs = mud::systems::run_game_systems(&mut ctx, inputs, Some(&engine));

    // Bob should see "Alice님이 동쪽으로 떠났습니다."
    let bob_msgs: Vec<_> = outputs.iter().filter(|o| o.session_id == sid2).collect();
    assert!(!bob_msgs.is_empty(), "Bob should see departure message");
    assert!(bob_msgs[0].text.contains("Alice") && bob_msgs[0].text.contains("떠났습니다"),
            "Bob departure msg: {}", bob_msgs[0].text);
}
