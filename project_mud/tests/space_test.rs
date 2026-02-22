//! Integration tests for space crate.

use ecs_adapter::EntityId;
use space::room_graph::{RoomExits, RoomGraphSpace};
use space::SpaceModel;

#[test]
fn full_lifecycle() {
    let mut space = RoomGraphSpace::new();
    let room_a = EntityId::new(100, 0);
    let room_b = EntityId::new(101, 0);
    let room_c = EntityId::new(102, 0);

    // Register rooms: A ↔ B ↔ C
    space.register_room(
        room_a,
        RoomExits {
            east: Some(room_b),
            ..Default::default()
        },
    );
    space.register_room(
        room_b,
        RoomExits {
            west: Some(room_a),
            east: Some(room_c),
            ..Default::default()
        },
    );
    space.register_room(room_c, RoomExits {
        west: Some(room_b),
        ..Default::default()
    });

    assert_eq!(space.room_count(), 3);

    // Place entities
    let e1 = EntityId::new(1, 0);
    let e2 = EntityId::new(2, 0);
    let e3 = EntityId::new(3, 0);

    space.place_entity(e1, room_a).unwrap();
    space.place_entity(e2, room_a).unwrap();
    space.place_entity(e3, room_b).unwrap();

    // e1 and e2 are in the same room
    let same_area = space.entities_in_same_area(e1).unwrap();
    assert!(same_area.contains(&e1));
    assert!(same_area.contains(&e2));
    assert!(!same_area.contains(&e3));

    // broadcast_targets returns same-room entities
    let targets = space.broadcast_targets(e1).unwrap();
    assert_eq!(targets.len(), 2);

    // Move e1 from A to B
    space.move_entity(e1, room_b).unwrap();
    assert_eq!(space.entity_room(e1), Some(room_b));

    // Now e1 and e3 are together
    let same_area = space.entities_in_same_area(e1).unwrap();
    assert!(same_area.contains(&e1));
    assert!(same_area.contains(&e3));

    // Remove e1
    space.remove_entity(e1).unwrap();
    assert_eq!(space.entity_room(e1), None);

    // e3 is alone in B now
    let same_area = space.entities_in_same_area(e3).unwrap();
    assert_eq!(same_area.len(), 1);
    assert!(same_area.contains(&e3));
}

#[test]
fn custom_exits() {
    let mut space = RoomGraphSpace::new();
    let room_a = EntityId::new(100, 0);
    let room_secret = EntityId::new(200, 0);

    let mut exits = RoomExits::default();
    exits.custom.insert("trapdoor".to_string(), room_secret);

    space.register_room(room_a, exits);
    space.register_room(room_secret, RoomExits::default());

    let neighbors = space.neighbors(room_a).unwrap();
    assert!(neighbors.contains(&room_secret));

    let entity = EntityId::new(1, 0);
    space.place_entity(entity, room_a).unwrap();
    space.move_entity(entity, room_secret).unwrap();
    assert_eq!(space.entity_room(entity), Some(room_secret));
}

#[test]
fn error_on_place_in_nonexistent_room() {
    let space = &mut RoomGraphSpace::new();
    let fake_room = EntityId::new(999, 0);
    let entity = EntityId::new(1, 0);
    assert!(space.place_entity(entity, fake_room).is_err());
}

#[test]
fn error_on_remove_nonexistent_entity() {
    let space = &mut RoomGraphSpace::new();
    let entity = EntityId::new(1, 0);
    assert!(space.remove_entity(entity).is_err());
}
