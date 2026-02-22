use serde::{Deserialize, Serialize};

use crate::grid_space::{GridSpace, GridSpaceSnapshot};
use crate::room_graph::{RoomGraphSpace, SpaceSnapshot};

/// Polymorphic space snapshot — supports both room-graph and grid models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpaceSnapshotData {
    /// Room-graph based space (MUD).
    RoomGraph(SpaceSnapshot),
    /// 2D coordinate grid space (MMO).
    Grid(GridSpaceSnapshot),
}

/// Trait for space models that can capture and restore snapshots.
pub trait SpaceSnapshotCapture {
    fn capture_snapshot(&self) -> SpaceSnapshotData;
    fn restore_snapshot(&mut self, data: SpaceSnapshotData) -> Result<(), String>;
}

impl SpaceSnapshotCapture for RoomGraphSpace {
    fn capture_snapshot(&self) -> SpaceSnapshotData {
        SpaceSnapshotData::RoomGraph(self.snapshot_state())
    }

    fn restore_snapshot(&mut self, data: SpaceSnapshotData) -> Result<(), String> {
        match data {
            SpaceSnapshotData::RoomGraph(snap) => {
                self.restore_from_snapshot(snap);
                Ok(())
            }
            SpaceSnapshotData::Grid(_) => {
                Err("cannot restore Grid snapshot into RoomGraphSpace".to_string())
            }
        }
    }
}

impl SpaceSnapshotCapture for GridSpace {
    fn capture_snapshot(&self) -> SpaceSnapshotData {
        SpaceSnapshotData::Grid(self.snapshot_state())
    }

    fn restore_snapshot(&mut self, data: SpaceSnapshotData) -> Result<(), String> {
        match data {
            SpaceSnapshotData::Grid(snap) => {
                self.restore_from_snapshot(snap);
                Ok(())
            }
            SpaceSnapshotData::RoomGraph(_) => {
                Err("cannot restore RoomGraph snapshot into GridSpace".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid_space::GridConfig;
    use crate::model::SpaceModel;
    use crate::room_graph::RoomExits;
    use ecs_adapter::EntityId;

    #[test]
    fn room_graph_snapshot_roundtrip() {
        let mut space = RoomGraphSpace::new();
        let room = EntityId::new(100, 0);
        space.register_room(room, RoomExits::default());
        let e1 = EntityId::new(1, 0);
        space.place_entity(e1, room).unwrap();

        let data = space.capture_snapshot();
        assert!(matches!(data, SpaceSnapshotData::RoomGraph(_)));

        let mut space2 = RoomGraphSpace::new();
        space2.restore_snapshot(data).unwrap();
        assert_eq!(space2.entity_room(e1), Some(room));
    }

    #[test]
    fn grid_snapshot_roundtrip() {
        let mut grid = GridSpace::new(GridConfig {
            width: 10,
            height: 10,
            origin_x: 0,
            origin_y: 0,
        });
        let e1 = EntityId::new(1, 0);
        grid.set_position(e1, 5, 5).unwrap();

        let data = grid.capture_snapshot();
        assert!(matches!(data, SpaceSnapshotData::Grid(_)));

        let mut grid2 = GridSpace::new(GridConfig::default());
        grid2.restore_snapshot(data).unwrap();
        assert_eq!(
            grid2.get_position(e1),
            Some(crate::grid_space::GridPos::new(5, 5))
        );
    }

    #[test]
    fn cross_type_restore_fails() {
        let grid = GridSpace::new(GridConfig::default());
        let grid_data = grid.capture_snapshot();

        let mut room_space = RoomGraphSpace::new();
        assert!(room_space.restore_snapshot(grid_data).is_err());

        let room_space2 = RoomGraphSpace::new();
        let room_data = room_space2.capture_snapshot();

        let mut grid2 = GridSpace::new(GridConfig::default());
        assert!(grid2.restore_snapshot(room_data).is_err());
    }
}
