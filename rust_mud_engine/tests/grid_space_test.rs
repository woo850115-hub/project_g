/// Integration tests for GridSpace lifecycle, bounds, radius queries, and snapshot persistence.
use ecs_adapter::EcsAdapter;
use persistence::manager::SnapshotManager;
use persistence::registry::PersistenceRegistry;
use persistence::snapshot;
use space::grid_space::{cell_to_entity_id, entity_id_to_cell, GridConfig, GridPos, GridSpace};
use space::SpaceModel;

fn make_grid(w: u32, h: u32) -> GridSpace {
    GridSpace::new(GridConfig {
        width: w,
        height: h,
        origin_x: 0,
        origin_y: 0,
    })
}

fn entity(idx: u32) -> ecs_adapter::EntityId {
    ecs_adapter::EntityId::new(idx, 0)
}

// --- Lifecycle tests ---

#[test]
fn place_move_remove_cycle() {
    let mut grid = make_grid(10, 10);
    let e1 = entity(1);
    let start = cell_to_entity_id(5, 5);
    let east = cell_to_entity_id(6, 5);

    // Place
    grid.place_entity(e1, start).unwrap();
    assert_eq!(grid.entity_room(e1), Some(start));
    assert_eq!(grid.get_position(e1), Some(GridPos::new(5, 5)));

    // Move to adjacent cell
    grid.move_entity(e1, east).unwrap();
    assert_eq!(grid.entity_room(e1), Some(east));
    assert_eq!(grid.get_position(e1), Some(GridPos::new(6, 5)));

    // Remove
    grid.remove_entity(e1).unwrap();
    assert_eq!(grid.entity_room(e1), None);
    assert_eq!(grid.get_position(e1), None);
    assert_eq!(grid.entity_count(), 0);
}

#[test]
fn multiple_entities_same_cell() {
    let mut grid = make_grid(10, 10);
    let cell = cell_to_entity_id(3, 3);

    let e1 = entity(1);
    let e2 = entity(2);
    let e3 = entity(3);
    grid.place_entity(e1, cell).unwrap();
    grid.place_entity(e2, cell).unwrap();
    grid.place_entity(e3, cell).unwrap();

    let same = grid.entities_in_same_area(e1).unwrap();
    assert_eq!(same.len(), 3);
    assert!(same.contains(&e1));
    assert!(same.contains(&e2));
    assert!(same.contains(&e3));
}

#[test]
fn entities_in_different_cells_not_in_same_area() {
    let mut grid = make_grid(10, 10);
    let e1 = entity(1);
    let e2 = entity(2);

    grid.place_entity(e1, cell_to_entity_id(0, 0)).unwrap();
    grid.place_entity(e2, cell_to_entity_id(9, 9)).unwrap();

    let same = grid.entities_in_same_area(e1).unwrap();
    assert_eq!(same, vec![e1]);
}

// --- Boundary tests ---

#[test]
fn move_out_of_bounds_rejected() {
    let mut grid = make_grid(10, 10);
    let e1 = entity(1);
    let corner = cell_to_entity_id(9, 9);
    grid.place_entity(e1, corner).unwrap();

    // Try to move east (x=10, out of bounds)
    let oob = cell_to_entity_id(10, 9);
    assert!(grid.move_entity(e1, oob).is_err());

    // Entity should still be at corner
    assert_eq!(grid.get_position(e1), Some(GridPos::new(9, 9)));
}

#[test]
fn place_out_of_bounds_rejected() {
    let mut grid = make_grid(10, 10);
    let oob = cell_to_entity_id(50, 50);
    assert!(grid.place_entity(entity(1), oob).is_err());
}

#[test]
fn negative_origin_grid() {
    let mut grid = GridSpace::new(GridConfig {
        width: 20,
        height: 20,
        origin_x: -10,
        origin_y: -10,
    });
    let e1 = entity(1);
    let cell = cell_to_entity_id(-5, -5);
    grid.place_entity(e1, cell).unwrap();
    assert_eq!(grid.get_position(e1), Some(GridPos::new(-5, -5)));

    // Move to (-4, -5)
    let target = cell_to_entity_id(-4, -5);
    grid.move_entity(e1, target).unwrap();
    assert_eq!(grid.get_position(e1), Some(GridPos::new(-4, -5)));
}

// --- Radius query tests ---

#[test]
fn radius_query_finds_nearby() {
    let mut grid = make_grid(20, 20);

    let e1 = entity(1);
    let e2 = entity(2);
    let e3 = entity(3);
    let e4 = entity(4);

    grid.set_position(e1, 10, 10).unwrap(); // center
    grid.set_position(e2, 11, 10).unwrap(); // distance 1
    grid.set_position(e3, 12, 10).unwrap(); // distance 2
    grid.set_position(e4, 15, 15).unwrap(); // far away

    let r1 = grid.entities_in_radius(10, 10, 1);
    assert!(r1.contains(&e1));
    assert!(r1.contains(&e2));
    assert!(!r1.contains(&e3));
    assert!(!r1.contains(&e4));

    let r2 = grid.entities_in_radius(10, 10, 2);
    assert!(r2.contains(&e1));
    assert!(r2.contains(&e2));
    assert!(r2.contains(&e3));
    assert!(!r2.contains(&e4));
}

#[test]
fn radius_query_empty() {
    let grid = make_grid(10, 10);
    let result = grid.entities_in_radius(5, 5, 3);
    assert!(result.is_empty());
}

// --- Neighbor tests ---

#[test]
fn neighbors_8_directions_center() {
    let grid = make_grid(10, 10);
    let center = cell_to_entity_id(5, 5);
    let neighbors = grid.neighbors(center).unwrap();
    assert_eq!(neighbors.len(), 8);

    // All neighbors should decode to valid positions
    for &n in &neighbors {
        let pos = entity_id_to_cell(n).unwrap();
        let dx = (pos.x - 5).abs();
        let dy = (pos.y - 5).abs();
        assert!(dx <= 1 && dy <= 1 && (dx + dy > 0));
    }
}

#[test]
fn neighbors_corner_3_directions() {
    let grid = make_grid(10, 10);
    let corner = cell_to_entity_id(0, 0);
    let neighbors = grid.neighbors(corner).unwrap();
    assert_eq!(neighbors.len(), 3);
}

// --- Snapshot disk persistence ---

#[test]
fn grid_snapshot_disk_roundtrip() {
    let dir = std::env::temp_dir().join("mud_test_grid_snapshot_disk");
    let _ = std::fs::remove_dir_all(&dir);

    let registry = PersistenceRegistry::new();
    let mut ecs = EcsAdapter::new();
    let mut grid = make_grid(50, 50);

    let e1 = ecs.spawn_entity();
    let e2 = ecs.spawn_entity();
    grid.set_position(e1, 10, 20).unwrap();
    grid.set_position(e2, 30, 40).unwrap();

    let snap = snapshot::capture(&ecs, &grid, 77, &registry);
    let mgr = SnapshotManager::new(&dir);
    mgr.save_to_disk(&snap).unwrap();

    // Load and restore
    let loaded = mgr.load_latest().unwrap();
    let mut ecs2 = EcsAdapter::new();
    let mut grid2 = GridSpace::new(GridConfig::default());
    let tick = snapshot::restore(loaded, &mut ecs2, &mut grid2, &registry).unwrap();

    assert_eq!(tick, 77);
    assert_eq!(grid2.get_position(e1), Some(GridPos::new(10, 20)));
    assert_eq!(grid2.get_position(e2), Some(GridPos::new(30, 40)));
    assert_eq!(grid2.entity_count(), 2);

    let _ = std::fs::remove_dir_all(&dir);
}

// --- Cell encoding ---

#[test]
fn cell_encoding_deterministic() {
    // Same coordinates always produce the same EntityId
    let a = cell_to_entity_id(42, -7);
    let b = cell_to_entity_id(42, -7);
    assert_eq!(a, b);

    // Different coordinates produce different EntityIds
    let c = cell_to_entity_id(43, -7);
    assert_ne!(a, c);
}

#[test]
fn set_position_updates_spatial_index() {
    let mut grid = make_grid(10, 10);
    let e1 = entity(1);
    let e2 = entity(2);

    grid.set_position(e1, 5, 5).unwrap();
    grid.set_position(e2, 5, 5).unwrap();

    // Both in same cell
    let same = grid.entities_in_same_area(e1).unwrap();
    assert_eq!(same.len(), 2);

    // Move e1 away via set_position
    grid.set_position(e1, 8, 8).unwrap();
    let same2 = grid.entities_in_same_area(e2).unwrap();
    assert_eq!(same2, vec![e2]);
}
