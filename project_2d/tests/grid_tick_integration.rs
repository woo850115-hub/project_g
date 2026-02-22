/// Integration tests for TickLoop<GridSpace>: basic stepping, EngineCommand processing, spawn+place+step.
use ecs_adapter::EntityId;
use engine_core::command::EngineCommand;
use engine_core::tick::{TickConfig, TickLoop};
use space::grid_space::{cell_to_entity_id, GridConfig, GridPos, GridSpace};
use space::SpaceModel;

fn make_tick_loop() -> TickLoop<GridSpace> {
    let config = TickConfig {
        tps: 30,
        max_ticks: 0,
    };
    let grid = GridSpace::new(GridConfig {
        width: 20,
        height: 20,
        origin_x: 0,
        origin_y: 0,
    });
    TickLoop::new(config, grid)
}

#[test]
fn grid_tick_loop_basic_step() {
    let mut tick_loop = make_tick_loop();
    let metrics = tick_loop.step();
    assert_eq!(metrics.tick_number, 1);
    assert_eq!(metrics.command_count, 0);
    assert_eq!(metrics.entity_count, 0);
}

#[test]
fn grid_tick_loop_multiple_steps() {
    let mut tick_loop = make_tick_loop();
    for i in 1..=5 {
        let metrics = tick_loop.step();
        assert_eq!(metrics.tick_number, i);
    }
    assert_eq!(tick_loop.current_tick, 5);
}

#[test]
fn grid_tick_loop_run_limited() {
    let config = TickConfig {
        tps: 1000, // fast for testing
        max_ticks: 10,
    };
    let grid = GridSpace::new(GridConfig {
        width: 10,
        height: 10,
        origin_x: 0,
        origin_y: 0,
    });
    let mut tick_loop = TickLoop::new(config, grid);
    let metrics = tick_loop.run();
    assert_eq!(metrics.len(), 10);
    assert_eq!(tick_loop.current_tick, 10);
}

#[test]
fn move_entity_via_engine_command() {
    let mut tick_loop = make_tick_loop();

    // Manually place an entity
    let e1 = tick_loop.ecs.spawn_entity();
    let start = cell_to_entity_id(5, 5);
    let target = cell_to_entity_id(6, 5);
    tick_loop.space.place_entity(e1, start).unwrap();

    // Queue a MoveEntity command
    tick_loop.commands.push(EngineCommand::MoveEntity {
        entity: e1,
        target_room: target,
    });

    // Step processes the command
    tick_loop.step();

    assert_eq!(
        tick_loop.space.get_position(e1),
        Some(GridPos::new(6, 5))
    );
}

#[test]
fn spawn_place_step_integration() {
    let mut tick_loop = make_tick_loop();

    // Spawn entity via ECS
    let e1 = tick_loop.ecs.spawn_entity();
    let cell = cell_to_entity_id(10, 10);
    tick_loop.space.place_entity(e1, cell).unwrap();

    assert_eq!(tick_loop.ecs.entity_count(), 1);
    assert_eq!(tick_loop.space.entity_count(), 1);
    assert_eq!(tick_loop.space.entity_room(e1), Some(cell));

    // Step a few times — entity stays put
    for _ in 0..3 {
        tick_loop.step();
    }

    assert_eq!(tick_loop.space.get_position(e1), Some(GridPos::new(10, 10)));
    assert_eq!(tick_loop.ecs.entity_count(), 1);
}

#[test]
fn destroy_entity_cleans_grid() {
    let mut tick_loop = make_tick_loop();

    let e1 = tick_loop.ecs.spawn_entity();
    let cell = cell_to_entity_id(3, 3);
    tick_loop.space.place_entity(e1, cell).unwrap();

    // Queue destroy command
    tick_loop.commands.push(EngineCommand::DestroyEntity { entity: e1 });
    tick_loop.step();

    // Entity removed from both ECS and grid
    assert_eq!(tick_loop.ecs.entity_count(), 0);
    assert_eq!(tick_loop.space.entity_count(), 0);
    assert_eq!(tick_loop.space.entity_room(e1), None);
}

#[test]
fn multiple_entities_move_in_same_tick() {
    let mut tick_loop = make_tick_loop();

    let e1 = tick_loop.ecs.spawn_entity();
    let e2 = tick_loop.ecs.spawn_entity();

    tick_loop
        .space
        .place_entity(e1, cell_to_entity_id(5, 5))
        .unwrap();
    tick_loop
        .space
        .place_entity(e2, cell_to_entity_id(8, 8))
        .unwrap();

    // Both entities move in the same tick
    tick_loop.commands.push(EngineCommand::MoveEntity {
        entity: e1,
        target_room: cell_to_entity_id(6, 5),
    });
    tick_loop.commands.push(EngineCommand::MoveEntity {
        entity: e2,
        target_room: cell_to_entity_id(9, 8),
    });

    tick_loop.step();

    assert_eq!(
        tick_loop.space.get_position(e1),
        Some(GridPos::new(6, 5))
    );
    assert_eq!(
        tick_loop.space.get_position(e2),
        Some(GridPos::new(9, 8))
    );
}

#[test]
fn invalid_move_command_ignored() {
    let mut tick_loop = make_tick_loop();

    let e1 = tick_loop.ecs.spawn_entity();
    tick_loop
        .space
        .place_entity(e1, cell_to_entity_id(0, 0))
        .unwrap();

    // Try to move 2 cells away — should fail but not crash
    tick_loop.commands.push(EngineCommand::MoveEntity {
        entity: e1,
        target_room: cell_to_entity_id(5, 5),
    });

    tick_loop.step();

    // Entity stays put
    assert_eq!(
        tick_loop.space.get_position(e1),
        Some(GridPos::new(0, 0))
    );
}

#[test]
fn move_nonexistent_entity_ignored() {
    let mut tick_loop = make_tick_loop();

    let fake = EntityId::new(999, 0);
    tick_loop.commands.push(EngineCommand::MoveEntity {
        entity: fake,
        target_room: cell_to_entity_id(5, 5),
    });

    // Should not panic
    tick_loop.step();
}
