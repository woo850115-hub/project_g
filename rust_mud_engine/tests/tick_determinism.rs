//! Determinism test: same seed → same state after every tick.

use ecs_adapter::EntityId;
use engine_core::command::EngineCommand;
use engine_core::tick::{TickConfig, TickLoop};
use space::room_graph::RoomExits;
use space::SpaceModel;

const NUM_ENTITIES: u32 = 100;
const NUM_ROOMS: u32 = 5;
const NUM_TICKS: u64 = 100;

/// Simple deterministic pseudo-random number generator (xorshift32).
struct Rng {
    state: u32,
}

impl Rng {
    fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }
}

fn setup_simulation(seed: u32) -> (TickLoop<space::RoomGraphSpace>, Vec<EntityId>, Vec<EntityId>) {
    let config = TickConfig {
        tps: 30,
        max_ticks: NUM_TICKS,
    };
    let mut tick_loop = TickLoop::new(config, space::RoomGraphSpace::new());
    let mut rng = Rng::new(seed);

    // Create rooms in a ring
    let rooms: Vec<EntityId> = (0..NUM_ROOMS).map(|i| EntityId::new(1000 + i, 0)).collect();
    for i in 0..NUM_ROOMS as usize {
        let next = (i + 1) % NUM_ROOMS as usize;
        let prev = (i + NUM_ROOMS as usize - 1) % NUM_ROOMS as usize;
        tick_loop.space.register_room(
            rooms[i],
            RoomExits {
                north: Some(rooms[next]),
                south: Some(rooms[prev]),
                ..Default::default()
            },
        );
    }

    // Spawn entities and place deterministically
    let mut entities = Vec::new();
    for _ in 0..NUM_ENTITIES {
        let eid = tick_loop.ecs.spawn_entity();
        let room_idx = (rng.next() as usize) % rooms.len();
        tick_loop.space.place_entity(eid, rooms[room_idx]).unwrap();
        entities.push(eid);
    }

    (tick_loop, entities, rooms)
}

/// Capture the state as a sorted list of (entity, room) pairs.
fn capture_state(tick_loop: &TickLoop<space::RoomGraphSpace>, entities: &[EntityId]) -> Vec<(EntityId, EntityId)> {
    let mut state: Vec<(EntityId, EntityId)> = entities
        .iter()
        .filter_map(|&e| tick_loop.space.entity_room(e).map(|r| (e, r)))
        .collect();
    state.sort();
    state
}

/// Run a full simulation and return states at every tick.
fn run_simulation(seed: u32) -> Vec<Vec<(EntityId, EntityId)>> {
    let (mut tick_loop, entities, _rooms) = setup_simulation(seed);
    let mut rng = Rng::new(seed);
    // Advance rng past the initial entity placement
    for _ in 0..NUM_ENTITIES {
        rng.next();
    }

    let mut states = Vec::new();

    for _tick in 0..NUM_TICKS {
        // Generate deterministic commands based on rng
        for &entity in &entities {
            let val = rng.next();
            if val % 5 == 0 {
                // 20% chance to move
                if let Some(current_room) = tick_loop.space.entity_room(entity) {
                    let neighbors = tick_loop.space.neighbors(current_room).unwrap();
                    if !neighbors.is_empty() {
                        let target_idx = (rng.next() as usize) % neighbors.len();
                        tick_loop.commands.push(EngineCommand::MoveEntity {
                            entity,
                            target_room: neighbors[target_idx],
                        });
                    }
                }
            }
        }

        tick_loop.step();
        states.push(capture_state(&tick_loop, &entities));
    }

    states
}

#[test]
fn determinism_same_seed_same_result() {
    let seed = 42;
    let states_a = run_simulation(seed);
    let states_b = run_simulation(seed);

    assert_eq!(states_a.len(), NUM_TICKS as usize);
    assert_eq!(states_b.len(), NUM_TICKS as usize);

    for tick in 0..NUM_TICKS as usize {
        assert_eq!(
            states_a[tick], states_b[tick],
            "state diverged at tick {}",
            tick
        );
    }
}

#[test]
fn determinism_different_seed_different_result() {
    let states_a = run_simulation(42);
    let states_b = run_simulation(99);

    // At least some ticks should differ (very high probability)
    let differs = states_a
        .iter()
        .zip(states_b.iter())
        .any(|(a, b)| a != b);
    assert!(differs, "different seeds should produce different states");
}
