use std::collections::BTreeMap;

use ecs_adapter::{EcsAdapter, EntityAllocator, EntityId};
use serde::{Deserialize, Serialize};
use space::snapshot::{SpaceSnapshotCapture, SpaceSnapshotData};

use crate::registry::PersistenceRegistry;

pub const SNAPSHOT_VERSION: u32 = 2;

/// Component data for a single entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySnapshot {
    pub entity_id: EntityId,
    pub components: BTreeMap<String, Vec<u8>>,
}

/// Full world snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub version: u32,
    pub tick: u64,
    pub allocator: EntityAllocator,
    pub entities: Vec<EntitySnapshot>,
    pub space: SpaceSnapshotData,
}

/// Capture a complete world snapshot from the current ECS and space state.
pub fn capture<S: SpaceSnapshotCapture>(
    ecs: &EcsAdapter,
    space: &S,
    tick: u64,
    registry: &PersistenceRegistry,
) -> WorldSnapshot {
    let allocator = ecs.allocator().clone();
    let all_entities = ecs.all_entities();

    let mut entities = Vec::new();
    for &eid in &all_entities {
        let mut comps = BTreeMap::new();
        for handler in registry.components() {
            if let Some(bytes) = handler.capture(ecs, eid) {
                comps.insert(handler.tag().to_string(), bytes);
            }
        }
        entities.push(EntitySnapshot {
            entity_id: eid,
            components: comps,
        });
    }

    let space_snap = space.capture_snapshot();

    WorldSnapshot {
        version: SNAPSHOT_VERSION,
        tick,
        allocator,
        entities,
        space: space_snap,
    }
}

/// Restore a world snapshot into the provided ECS and space.
/// This clears the existing ECS and space, then rebuilds from the snapshot.
pub fn restore<S: SpaceSnapshotCapture>(
    snapshot: WorldSnapshot,
    ecs: &mut EcsAdapter,
    space: &mut S,
    registry: &PersistenceRegistry,
) -> Result<u64, crate::error::PersistenceError> {
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(crate::error::PersistenceError::VersionMismatch {
            expected: SNAPSHOT_VERSION,
            got: snapshot.version,
        });
    }

    // Reset ECS with fresh bevy World, then restore allocator state
    *ecs = EcsAdapter::new();
    *ecs.allocator_mut() = snapshot.allocator;

    // Build a lookup from tag -> handler for efficient restore
    let handler_map: BTreeMap<&str, &dyn crate::registry::PersistentComponent> = registry
        .components()
        .iter()
        .map(|h| (h.tag(), h.as_ref()))
        .collect();

    // Spawn entities with their original IDs and restore components
    for entity_snap in &snapshot.entities {
        let eid = entity_snap.entity_id;
        ecs.spawn_entity_with_id(eid)
            .map_err(|e| crate::error::PersistenceError::Corrupt(e.to_string()))?;

        for (tag, data) in &entity_snap.components {
            if let Some(handler) = handler_map.get(tag.as_str()) {
                handler.restore(ecs, eid, data)?;
            } else {
                tracing::warn!("Unknown component tag during restore: {}", tag);
            }
        }
    }

    // Restore space
    space
        .restore_snapshot(snapshot.space)
        .map_err(crate::error::PersistenceError::Corrupt)?;

    Ok(snapshot.tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{PersistenceRegistry, PersistentComponent};
    use ecs_adapter::Component;
    use serde::{Deserialize, Serialize};
    use space::SpaceModel;

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestName(String);

    #[derive(Component, Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestHealth {
        current: i32,
        max: i32,
    }

    struct TestNameHandler;
    impl PersistentComponent for TestNameHandler {
        fn tag(&self) -> &str {
            "TestName"
        }
        fn capture(&self, ecs: &EcsAdapter, eid: EntityId) -> Option<Vec<u8>> {
            ecs.get_component::<TestName>(eid)
                .ok()
                .and_then(|c| bincode::serialize(c).ok())
        }
        fn restore(
            &self,
            ecs: &mut EcsAdapter,
            eid: EntityId,
            data: &[u8],
        ) -> Result<(), crate::error::PersistenceError> {
            let c: TestName = bincode::deserialize(data)?;
            ecs.set_component(eid, c)
                .map_err(|e| crate::error::PersistenceError::Corrupt(e.to_string()))
        }
    }

    struct TestHealthHandler;
    impl PersistentComponent for TestHealthHandler {
        fn tag(&self) -> &str {
            "TestHealth"
        }
        fn capture(&self, ecs: &EcsAdapter, eid: EntityId) -> Option<Vec<u8>> {
            ecs.get_component::<TestHealth>(eid)
                .ok()
                .and_then(|c| bincode::serialize(c).ok())
        }
        fn restore(
            &self,
            ecs: &mut EcsAdapter,
            eid: EntityId,
            data: &[u8],
        ) -> Result<(), crate::error::PersistenceError> {
            let c: TestHealth = bincode::deserialize(data)?;
            ecs.set_component(eid, c)
                .map_err(|e| crate::error::PersistenceError::Corrupt(e.to_string()))
        }
    }

    fn test_registry() -> PersistenceRegistry {
        let mut reg = PersistenceRegistry::new();
        reg.register(Box::new(TestNameHandler));
        reg.register(Box::new(TestHealthHandler));
        reg
    }

    #[test]
    fn capture_restore_roundtrip() {
        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let mut space = space::RoomGraphSpace::new();

        // Create a room
        let room = ecs.spawn_entity();
        space.register_room(room, space::room_graph::RoomExits::default());

        // Create an entity with components
        let e1 = ecs.spawn_entity();
        ecs.set_component(e1, TestName("Hero".to_string())).unwrap();
        ecs.set_component(e1, TestHealth { current: 80, max: 100 }).unwrap();
        space.place_entity(e1, room).unwrap();

        let snap = capture(&ecs, &space, 42, &registry);
        assert_eq!(snap.version, SNAPSHOT_VERSION);
        assert_eq!(snap.tick, 42);

        let mut ecs2 = EcsAdapter::new();
        let mut space2 = space::RoomGraphSpace::new();
        let tick = restore(snap, &mut ecs2, &mut space2, &registry).unwrap();
        assert_eq!(tick, 42);

        // Verify component data
        let name = ecs2.get_component::<TestName>(e1).unwrap();
        assert_eq!(name.0, "Hero");
        let hp = ecs2.get_component::<TestHealth>(e1).unwrap();
        assert_eq!(hp.current, 80);
        assert_eq!(hp.max, 100);

        // Verify space
        assert_eq!(space2.entity_room(e1), Some(room));
    }

    #[test]
    fn version_mismatch_rejected() {
        let registry = test_registry();
        let ecs = EcsAdapter::new();
        let space = space::RoomGraphSpace::new();

        let mut snap = capture(&ecs, &space, 1, &registry);
        snap.version = 999;

        let mut ecs2 = EcsAdapter::new();
        let mut space2 = space::RoomGraphSpace::new();
        let result = restore(snap, &mut ecs2, &mut space2, &registry);
        assert!(result.is_err());
    }

    #[test]
    fn snapshot_preserves_entity_count() {
        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let space = space::RoomGraphSpace::new();

        let _e1 = ecs.spawn_entity();
        let _e2 = ecs.spawn_entity();
        let _e3 = ecs.spawn_entity();

        let original_count = ecs.entity_count();
        let snap = capture(&ecs, &space, 10, &registry);

        let mut ecs2 = EcsAdapter::new();
        let mut space2 = space::RoomGraphSpace::new();
        restore(snap, &mut ecs2, &mut space2, &registry).unwrap();

        assert_eq!(ecs2.entity_count(), original_count);
    }

    #[test]
    fn snapshot_bincode_roundtrip() {
        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let space = space::RoomGraphSpace::new();

        let e1 = ecs.spawn_entity();
        ecs.set_component(e1, TestName("Test".to_string())).unwrap();

        let snap = capture(&ecs, &space, 100, &registry);
        let bytes = bincode::serialize(&snap).unwrap();
        let decoded: WorldSnapshot = bincode::deserialize(&bytes).unwrap();

        assert_eq!(decoded.version, snap.version);
        assert_eq!(decoded.tick, snap.tick);
        assert_eq!(decoded.entities.len(), snap.entities.len());
    }

    #[test]
    fn grid_space_capture_restore() {
        let registry = test_registry();
        let mut ecs = EcsAdapter::new();
        let mut grid = space::GridSpace::new(space::grid_space::GridConfig {
            width: 50,
            height: 50,
            origin_x: 0,
            origin_y: 0,
        });

        let e1 = ecs.spawn_entity();
        ecs.set_component(e1, TestName("GridHero".to_string())).unwrap();
        grid.set_position(e1, 10, 20).unwrap();

        let snap = capture(&ecs, &grid, 55, &registry);
        assert!(matches!(snap.space, SpaceSnapshotData::Grid(_)));

        let mut ecs2 = EcsAdapter::new();
        let mut grid2 = space::GridSpace::new(space::grid_space::GridConfig::default());
        let tick = restore(snap, &mut ecs2, &mut grid2, &registry).unwrap();
        assert_eq!(tick, 55);

        let name = ecs2.get_component::<TestName>(e1).unwrap();
        assert_eq!(name.0, "GridHero");
        assert_eq!(
            grid2.get_position(e1),
            Some(space::grid_space::GridPos::new(10, 20))
        );
    }
}
