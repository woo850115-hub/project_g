//! Integration test: 100 entities, 5 rooms, 300 ticks simulation.

use ecs_adapter::EntityId;
use engine_core::command::{CommandStream, EngineCommand};
use engine_core::tick::{TickConfig, TickLoop};
use space::room_graph::{RoomExits, RoomGraphSpace};
use space::SpaceModel;

/// Set up a ring of 5 rooms: 0→1→2→3→4→0
fn setup_ring_rooms(space: &mut RoomGraphSpace) -> Vec<EntityId> {
    let rooms: Vec<EntityId> = (0..5).map(|i| EntityId::new(1000 + i, 0)).collect();

    for i in 0..5 {
        let next = (i + 1) % 5;
        let prev = (i + 4) % 5;
        space.register_room(
            rooms[i],
            RoomExits {
                north: Some(rooms[next]),
                south: Some(rooms[prev]),
                ..Default::default()
            },
        );
    }

    rooms
}

#[test]
fn simulation_100_entities_5_rooms_300_ticks() {
    let config = TickConfig {
        tps: 30,
        max_ticks: 300,
    };
    let mut tick_loop = TickLoop::new(config, RoomGraphSpace::new());

    // Setup rooms
    let rooms = setup_ring_rooms(&mut tick_loop.space);

    // Spawn 100 entities and place them in rooms (round-robin)
    let mut entities = Vec::new();
    for i in 0..100u32 {
        let eid = tick_loop.ecs.spawn_entity();
        let room_idx = (i as usize) % rooms.len();
        tick_loop.space.place_entity(eid, rooms[room_idx]).unwrap();
        entities.push(eid);
    }

    assert_eq!(tick_loop.ecs.entity_count(), 100);

    // Run 300 ticks with movement commands
    let mut total_duration_us: u128 = 0;
    for tick in 0..300u64 {
        // Every tick, move some entities to the next room
        for (i, &entity) in entities.iter().enumerate() {
            if (tick as usize + i) % 7 == 0 {
                // Only move if entity is still in a room
                if let Some(current_room) = tick_loop.space.entity_room(entity) {
                    let neighbors = tick_loop.space.neighbors(current_room).unwrap();
                    if let Some(&target) = neighbors.first() {
                        tick_loop.commands.push(EngineCommand::MoveEntity {
                            entity,
                            target_room: target,
                        });
                    }
                }
            }
        }

        let metrics = tick_loop.step();
        total_duration_us += metrics.duration_us;
    }

    // Verify: still have 100 entities
    assert_eq!(tick_loop.ecs.entity_count(), 100);
    assert_eq!(tick_loop.current_tick, 300);

    // Verify: average tick < 33ms (33000 us)
    let avg_us = total_duration_us / 300;
    assert!(
        avg_us < 33_000,
        "average tick duration {}us exceeds 33ms budget",
        avg_us
    );

    // Verify: all entities are in some room
    for &entity in &entities {
        assert!(
            tick_loop.space.entity_room(entity).is_some(),
            "entity {} not in any room",
            entity
        );
    }
}

#[test]
fn command_stream_lww_integration() {
    let mut stream = CommandStream::new();
    let entity = EntityId::new(1, 0);
    let cid = ecs_adapter::ComponentId(10);

    // Push multiple conflicting commands
    for i in 0..10 {
        stream.push(EngineCommand::SetComponent {
            entity,
            component_id: cid,
            data: vec![i],
        });
    }

    let resolved = stream.resolve();
    // Only 1 SetComponent should survive (LWW)
    let set_cmds: Vec<_> = resolved
        .commands
        .iter()
        .filter(|c| matches!(c, EngineCommand::SetComponent { .. }))
        .collect();
    assert_eq!(set_cmds.len(), 1);
    if let EngineCommand::SetComponent { data, .. } = &set_cmds[0] {
        assert_eq!(data, &vec![9]); // Last writer (i=9) wins
    }
}
