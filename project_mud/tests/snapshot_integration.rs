/// Integration test: World creation -> Snapshot save -> Restore -> State verification.
use std::path::Path;

use ecs_adapter::EcsAdapter;
use mud::components::*;
use mud::persistence_setup::register_mud_components;
use mud::script_setup::register_mud_script_components;
use mud::session::SessionManager;
use persistence::manager::SnapshotManager;
use persistence::registry::PersistenceRegistry;
use persistence::snapshot;
use scripting::engine::{ScriptContext, ScriptEngine};
use scripting::ScriptConfig;
use space::{RoomGraphSpace, SpaceModel};

fn scripts_dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts"))
}

fn test_registry() -> PersistenceRegistry {
    let mut registry = PersistenceRegistry::new();
    register_mud_components(&mut registry);
    registry
}

/// Create world via Lua scripts (same as production flow).
fn create_world_via_lua(ecs: &mut EcsAdapter, space: &mut RoomGraphSpace) -> ScriptEngine {
    let mut engine = ScriptEngine::new(ScriptConfig::default()).unwrap();
    register_mud_script_components(engine.component_registry_mut());
    engine.load_directory(scripts_dir()).unwrap();

    let sessions = SessionManager::new();
    let mut ctx = ScriptContext {
        ecs,
        space,
        sessions: &sessions,
        tick: 0,
    };
    engine.run_on_init(&mut ctx).unwrap();
    engine
}

/// Find an entity by Name component.
fn find_entity_by_name(ecs: &EcsAdapter, name: &str) -> Option<ecs_adapter::EntityId> {
    ecs.entities_with::<Name>()
        .into_iter()
        .find(|&eid| {
            ecs.get_component::<Name>(eid)
                .map(|n| n.0 == name)
                .unwrap_or(false)
        })
}

#[test]
fn full_snapshot_cycle() {
    let registry = test_registry();
    let mut ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    create_world_via_lua(&mut ecs, &mut space);

    let goblin = find_entity_by_name(&ecs, "고블린").unwrap();
    let spawn_room = find_entity_by_name(&ecs, "시작의 방").unwrap();
    let market = find_entity_by_name(&ecs, "시장 광장").unwrap();
    let dungeon_floor1 = find_entity_by_name(&ecs, "던전 1층").unwrap();

    // Modify some state: damage the goblin
    let hp = ecs.get_component::<Health>(goblin).unwrap().clone();
    ecs.set_component(
        goblin,
        Health {
            current: hp.current - 10,
            max: hp.max,
        },
    )
    .unwrap();

    // Capture
    let tick = 42;
    let snap = snapshot::capture(&ecs, &space, tick, &registry);
    assert_eq!(snap.tick, 42);
    assert_eq!(snap.entities.len(), ecs.entity_count());

    // Restore into fresh state
    let mut ecs2 = EcsAdapter::new();
    let mut space2 = RoomGraphSpace::new();
    let restored_tick = snapshot::restore(snap, &mut ecs2, &mut space2, &registry).unwrap();

    assert_eq!(restored_tick, 42);

    // Verify room count
    assert_eq!(space2.room_count(), 6);

    // Verify goblin health was preserved (damaged)
    let hp2 = ecs2.get_component::<Health>(goblin).unwrap();
    assert_eq!(hp2.current, 20);
    assert_eq!(hp2.max, 30);

    // Verify goblin is still in dungeon floor 1
    assert_eq!(space2.entity_room(goblin), Some(dungeon_floor1));

    // Verify room names
    let name = ecs2.get_component::<Name>(spawn_room).unwrap();
    assert_eq!(name.0, "시작의 방");

    let name = ecs2.get_component::<Name>(market).unwrap();
    assert_eq!(name.0, "시장 광장");
}

#[test]
fn snapshot_disk_persistence() {
    let registry = test_registry();
    let dir = std::env::temp_dir().join("mud_test_snapshot_disk_integ");
    let _ = std::fs::remove_dir_all(&dir);

    let mut ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    create_world_via_lua(&mut ecs, &mut space);

    let goblin = find_entity_by_name(&ecs, "고블린").unwrap();
    let potion = find_entity_by_name(&ecs, "치유 물약").unwrap();

    let snap = snapshot::capture(&ecs, &space, 100, &registry);
    let mgr = SnapshotManager::new(&dir);
    mgr.save_to_disk(&snap).unwrap();

    // Load and restore
    let loaded = mgr.load_latest().unwrap();
    let mut ecs2 = EcsAdapter::new();
    let mut space2 = RoomGraphSpace::new();
    let tick = snapshot::restore(loaded, &mut ecs2, &mut space2, &registry).unwrap();
    assert_eq!(tick, 100);

    // Verify everything
    assert_eq!(space2.room_count(), 6);
    assert!(ecs2.has_component::<NpcTag>(goblin));
    assert!(ecs2.has_component::<ItemTag>(potion));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn snapshot_with_player_entity() {
    let registry = test_registry();
    let mut ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    create_world_via_lua(&mut ecs, &mut space);

    let market = find_entity_by_name(&ecs, "시장 광장").unwrap();

    // Create a player entity
    let player = ecs.spawn_entity();
    ecs.set_component(player, Name("TestPlayer".to_string())).unwrap();
    ecs.set_component(player, PlayerTag).unwrap();
    ecs.set_component(player, Health { current: 80, max: 100 }).unwrap();
    ecs.set_component(player, Attack(12)).unwrap();
    ecs.set_component(player, Defense(5)).unwrap();
    let sword = ecs.spawn_entity();
    ecs.set_component(sword, Name("Magic Sword".to_string())).unwrap();
    ecs.set_component(sword, ItemTag).unwrap();
    ecs.set_component(player, Inventory { items: vec![sword] }).unwrap();
    space.place_entity(player, market).unwrap();

    // Snapshot
    let snap = snapshot::capture(&ecs, &space, 50, &registry);

    // Restore
    let mut ecs2 = EcsAdapter::new();
    let mut space2 = RoomGraphSpace::new();
    snapshot::restore(snap, &mut ecs2, &mut space2, &registry).unwrap();

    // Verify player
    let name = ecs2.get_component::<Name>(player).unwrap();
    assert_eq!(name.0, "TestPlayer");
    assert!(ecs2.has_component::<PlayerTag>(player));

    let hp = ecs2.get_component::<Health>(player).unwrap();
    assert_eq!(hp.current, 80);

    let inv = ecs2.get_component::<Inventory>(player).unwrap();
    assert_eq!(inv.items.len(), 1);
    assert_eq!(inv.items[0], sword);

    // Verify player location
    assert_eq!(space2.entity_room(player), Some(market));
}

#[test]
fn snapshot_version_mismatch() {
    let registry = test_registry();
    let mut ecs = EcsAdapter::new();
    let mut space = RoomGraphSpace::new();
    create_world_via_lua(&mut ecs, &mut space);

    let mut snap = snapshot::capture(&ecs, &space, 1, &registry);
    snap.version = 999;

    let mut ecs2 = EcsAdapter::new();
    let mut space2 = RoomGraphSpace::new();
    let result = snapshot::restore(snap, &mut ecs2, &mut space2, &registry);
    assert!(result.is_err());
}
