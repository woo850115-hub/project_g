use std::collections::{BTreeMap, BTreeSet};

use ecs_adapter::EntityId;
use serde::{Deserialize, Serialize};

use crate::model::{MoveError, SpaceModel};

/// Sentinel generation value for synthetic cell EntityIds.
/// EntityAllocator never produces this generation, so no collision with real entities.
const GRID_CELL_GENERATION: u32 = u32::MAX;

/// 2D integer coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// Configuration for a GridSpace instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    /// Grid width in cells.
    pub width: u32,
    /// Grid height in cells.
    pub height: u32,
    /// Minimum X coordinate (left edge).
    pub origin_x: i32,
    /// Minimum Y coordinate (top edge).
    pub origin_y: i32,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            width: 100,
            height: 100,
            origin_x: 0,
            origin_y: 0,
        }
    }
}

/// Encode a cell coordinate into a synthetic EntityId for SpaceModel compatibility.
///
/// Coordinate range: i16::MIN..=i16::MAX per axis (-32768..32767).
/// The generation is set to `u32::MAX` to avoid collisions with real ECS entities.
pub fn cell_to_entity_id(x: i32, y: i32) -> EntityId {
    let ux = (x as i64 - i16::MIN as i64) as u32; // 0..65535
    let uy = (y as i64 - i16::MIN as i64) as u32;
    EntityId::new((uy << 16) | (ux & 0xFFFF), GRID_CELL_GENERATION)
}

/// Decode a synthetic cell EntityId back into coordinates.
/// Returns None if the EntityId is not a grid cell (wrong generation).
pub fn entity_id_to_cell(id: EntityId) -> Option<GridPos> {
    if id.generation != GRID_CELL_GENERATION {
        return None;
    }
    let ux = (id.index & 0xFFFF) as i64;
    let uy = ((id.index >> 16) & 0xFFFF) as i64;
    let x = (ux + i16::MIN as i64) as i32;
    let y = (uy + i16::MIN as i64) as i32;
    Some(GridPos::new(x, y))
}

/// 2D coordinate-based spatial model.
///
/// Entities are placed on integer grid cells. The grid has fixed bounds
/// defined by `GridConfig`. All internal structures use BTreeMap/BTreeSet
/// for deterministic iteration order.
#[derive(Debug)]
pub struct GridSpace {
    config: GridConfig,
    /// Entity → position mapping.
    entity_to_pos: BTreeMap<EntityId, GridPos>,
    /// Spatial index: position → set of entities at that cell.
    cell_occupants: BTreeMap<GridPos, BTreeSet<EntityId>>,
}

impl GridSpace {
    pub fn new(config: GridConfig) -> Self {
        Self {
            config,
            entity_to_pos: BTreeMap::new(),
            cell_occupants: BTreeMap::new(),
        }
    }

    /// Get the grid configuration.
    pub fn config(&self) -> &GridConfig {
        &self.config
    }

    /// Check if a coordinate is within grid bounds.
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= self.config.origin_x
            && x < self.config.origin_x + self.config.width as i32
            && y >= self.config.origin_y
            && y < self.config.origin_y + self.config.height as i32
    }

    /// Get the position of an entity.
    pub fn get_position(&self, entity: EntityId) -> Option<GridPos> {
        self.entity_to_pos.get(&entity).copied()
    }

    /// Set (teleport) an entity to an arbitrary in-bounds position.
    /// If the entity is already placed, it is moved; otherwise it is placed.
    pub fn set_position(&mut self, entity: EntityId, x: i32, y: i32) -> Result<(), MoveError> {
        if !self.in_bounds(x, y) {
            return Err(MoveError::OutOfBounds { x, y });
        }
        let new_pos = GridPos::new(x, y);

        // Remove from old cell if present
        if let Some(old_pos) = self.entity_to_pos.get(&entity).copied() {
            if let Some(set) = self.cell_occupants.get_mut(&old_pos) {
                set.remove(&entity);
                if set.is_empty() {
                    self.cell_occupants.remove(&old_pos);
                }
            }
        }

        self.entity_to_pos.insert(entity, new_pos);
        self.cell_occupants
            .entry(new_pos)
            .or_default()
            .insert(entity);
        Ok(())
    }

    /// Move an entity to a specific position (must be adjacent — Chebyshev distance 1).
    pub fn move_to(&mut self, entity: EntityId, x: i32, y: i32) -> Result<(), MoveError> {
        let current = self
            .entity_to_pos
            .get(&entity)
            .copied()
            .ok_or(MoveError::EntityNotInRoom(entity))?;

        if !self.in_bounds(x, y) {
            return Err(MoveError::OutOfBounds { x, y });
        }

        let dx = (x - current.x).abs();
        let dy = (y - current.y).abs();
        if dx > 1 || dy > 1 || (dx == 0 && dy == 0) {
            let target = cell_to_entity_id(x, y);
            let from = cell_to_entity_id(current.x, current.y);
            return Err(MoveError::NoExit {
                from,
                to: target,
            });
        }

        let new_pos = GridPos::new(x, y);

        // Remove from old cell
        if let Some(set) = self.cell_occupants.get_mut(&current) {
            set.remove(&entity);
            if set.is_empty() {
                self.cell_occupants.remove(&current);
            }
        }

        self.entity_to_pos.insert(entity, new_pos);
        self.cell_occupants
            .entry(new_pos)
            .or_default()
            .insert(entity);
        Ok(())
    }

    /// Find all entities within a given radius (Chebyshev distance) of a point.
    /// Results are sorted by EntityId for determinism.
    pub fn entities_in_radius(&self, x: i32, y: i32, radius: u32) -> Vec<EntityId> {
        let r = radius as i32;
        let mut result = Vec::new();

        let min_x = x.saturating_sub(r);
        let max_x = x.saturating_add(r);
        let min_y = y.saturating_sub(r);
        let max_y = y.saturating_add(r);

        let range_start = GridPos::new(min_x, min_y);
        let range_end = GridPos::new(max_x + 1, max_y + 1);

        for (pos, entities) in self.cell_occupants.range(range_start..range_end) {
            if pos.x >= min_x && pos.x <= max_x && pos.y >= min_y && pos.y <= max_y {
                result.extend(entities.iter());
            }
        }

        result.sort();
        result
    }

    /// Get all entity positions (for state broadcast).
    pub fn all_entity_positions(&self) -> &BTreeMap<EntityId, GridPos> {
        &self.entity_to_pos
    }

    /// Number of entities currently placed in the grid.
    pub fn entity_count(&self) -> usize {
        self.entity_to_pos.len()
    }

    /// Capture the full grid state as a serializable snapshot.
    pub fn snapshot_state(&self) -> GridSpaceSnapshot {
        let mut entities = Vec::new();
        for (&entity, &pos) in &self.entity_to_pos {
            entities.push(GridEntitySnapshot { entity, pos });
        }
        GridSpaceSnapshot {
            config: self.config.clone(),
            entities,
        }
    }

    /// Restore grid state from a snapshot, replacing all current data.
    pub fn restore_from_snapshot(&mut self, snapshot: GridSpaceSnapshot) {
        self.config = snapshot.config;
        self.entity_to_pos.clear();
        self.cell_occupants.clear();

        for entry in snapshot.entities {
            self.entity_to_pos.insert(entry.entity, entry.pos);
            self.cell_occupants
                .entry(entry.pos)
                .or_default()
                .insert(entry.entity);
        }
    }
}

impl SpaceModel for GridSpace {
    fn entity_room(&self, entity: EntityId) -> Option<EntityId> {
        self.entity_to_pos
            .get(&entity)
            .map(|pos| cell_to_entity_id(pos.x, pos.y))
    }

    fn entities_in_same_area(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError> {
        let pos = self
            .entity_to_pos
            .get(&entity)
            .ok_or(MoveError::EntityNotInRoom(entity))?;
        let mut result: Vec<_> = self
            .cell_occupants
            .get(pos)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        result.sort();
        Ok(result)
    }

    fn neighbors(&self, cell_id: EntityId) -> Result<Vec<EntityId>, MoveError> {
        let pos = entity_id_to_cell(cell_id).ok_or(MoveError::RoomNotFound(cell_id))?;

        let mut result = Vec::new();
        for dy in -1..=1_i32 {
            for dx in -1..=1_i32 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = pos.x + dx;
                let ny = pos.y + dy;
                if self.in_bounds(nx, ny) {
                    result.push(cell_to_entity_id(nx, ny));
                }
            }
        }
        result.sort();
        Ok(result)
    }

    fn move_entity(&mut self, entity: EntityId, target_cell: EntityId) -> Result<(), MoveError> {
        let target_pos =
            entity_id_to_cell(target_cell).ok_or(MoveError::RoomNotFound(target_cell))?;
        self.move_to(entity, target_pos.x, target_pos.y)
    }

    fn broadcast_targets(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError> {
        self.entities_in_same_area(entity)
    }

    fn place_entity(&mut self, entity: EntityId, cell_id: EntityId) -> Result<(), MoveError> {
        if self.entity_to_pos.contains_key(&entity) {
            return Err(MoveError::AlreadyPlaced(entity));
        }
        let pos = entity_id_to_cell(cell_id).ok_or(MoveError::RoomNotFound(cell_id))?;
        if !self.in_bounds(pos.x, pos.y) {
            return Err(MoveError::OutOfBounds { x: pos.x, y: pos.y });
        }
        self.entity_to_pos.insert(entity, pos);
        self.cell_occupants
            .entry(pos)
            .or_default()
            .insert(entity);
        Ok(())
    }

    fn remove_entity(&mut self, entity: EntityId) -> Result<(), MoveError> {
        let pos = self
            .entity_to_pos
            .remove(&entity)
            .ok_or(MoveError::EntityNotInRoom(entity))?;
        if let Some(set) = self.cell_occupants.get_mut(&pos) {
            set.remove(&entity);
            if set.is_empty() {
                self.cell_occupants.remove(&pos);
            }
        }
        Ok(())
    }
}

/// Serializable snapshot of a single entity's grid position.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridEntitySnapshot {
    pub entity: EntityId,
    pub pos: GridPos,
}

/// Serializable snapshot of the entire grid space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridSpaceSnapshot {
    pub config: GridConfig,
    pub entities: Vec<GridEntitySnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_grid() -> GridSpace {
        GridSpace::new(GridConfig {
            width: 10,
            height: 10,
            origin_x: 0,
            origin_y: 0,
        })
    }

    fn entity(idx: u32) -> EntityId {
        EntityId::new(idx, 0)
    }

    // --- cell_to_entity_id / entity_id_to_cell ---

    #[test]
    fn cell_encoding_roundtrip_origin() {
        let id = cell_to_entity_id(0, 0);
        let pos = entity_id_to_cell(id).unwrap();
        assert_eq!(pos, GridPos::new(0, 0));
    }

    #[test]
    fn cell_encoding_roundtrip_positive() {
        let id = cell_to_entity_id(100, 200);
        let pos = entity_id_to_cell(id).unwrap();
        assert_eq!(pos, GridPos::new(100, 200));
    }

    #[test]
    fn cell_encoding_roundtrip_negative() {
        let id = cell_to_entity_id(-50, -100);
        let pos = entity_id_to_cell(id).unwrap();
        assert_eq!(pos, GridPos::new(-50, -100));
    }

    #[test]
    fn cell_encoding_roundtrip_extremes() {
        for &(x, y) in &[
            (i16::MIN as i32, i16::MIN as i32),
            (i16::MAX as i32, i16::MAX as i32),
            (i16::MIN as i32, i16::MAX as i32),
            (i16::MAX as i32, i16::MIN as i32),
        ] {
            let id = cell_to_entity_id(x, y);
            let pos = entity_id_to_cell(id).unwrap();
            assert_eq!(pos, GridPos::new(x, y), "failed for ({}, {})", x, y);
        }
    }

    #[test]
    fn cell_encoding_uses_max_generation() {
        let id = cell_to_entity_id(0, 0);
        assert_eq!(id.generation, u32::MAX);
    }

    #[test]
    fn entity_id_to_cell_rejects_non_grid() {
        let real_entity = EntityId::new(42, 0);
        assert!(entity_id_to_cell(real_entity).is_none());
    }

    // --- in_bounds ---

    #[test]
    fn in_bounds_basic() {
        let grid = default_grid();
        assert!(grid.in_bounds(0, 0));
        assert!(grid.in_bounds(9, 9));
        assert!(!grid.in_bounds(10, 0));
        assert!(!grid.in_bounds(0, 10));
        assert!(!grid.in_bounds(-1, 0));
    }

    #[test]
    fn in_bounds_with_negative_origin() {
        let grid = GridSpace::new(GridConfig {
            width: 20,
            height: 20,
            origin_x: -10,
            origin_y: -10,
        });
        assert!(grid.in_bounds(-10, -10));
        assert!(grid.in_bounds(9, 9));
        assert!(!grid.in_bounds(10, 10));
        assert!(!grid.in_bounds(-11, 0));
    }

    // --- place_entity / entity_room ---

    #[test]
    fn place_entity_and_query() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(3, 4);

        grid.place_entity(e1, cell).unwrap();
        assert_eq!(grid.entity_room(e1), Some(cell));
        assert_eq!(grid.get_position(e1), Some(GridPos::new(3, 4)));
    }

    #[test]
    fn place_entity_double_fails() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(0, 0);

        grid.place_entity(e1, cell).unwrap();
        assert!(grid.place_entity(e1, cell).is_err());
    }

    #[test]
    fn place_entity_out_of_bounds_fails() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(50, 50); // grid is 10x10 at origin 0,0

        assert!(grid.place_entity(e1, cell).is_err());
    }

    #[test]
    fn place_entity_invalid_cell_id() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let not_a_cell = EntityId::new(42, 0); // not generation u32::MAX

        assert!(grid.place_entity(e1, not_a_cell).is_err());
    }

    // --- remove_entity ---

    #[test]
    fn remove_entity_basic() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(0, 0);

        grid.place_entity(e1, cell).unwrap();
        grid.remove_entity(e1).unwrap();
        assert_eq!(grid.entity_room(e1), None);
        assert_eq!(grid.get_position(e1), None);
    }

    #[test]
    fn remove_entity_not_placed_fails() {
        let mut grid = default_grid();
        assert!(grid.remove_entity(entity(99)).is_err());
    }

    // --- move_entity (SpaceModel) ---

    #[test]
    fn move_entity_to_neighbor() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let start = cell_to_entity_id(5, 5);
        let target = cell_to_entity_id(6, 5); // east neighbor

        grid.place_entity(e1, start).unwrap();
        grid.move_entity(e1, target).unwrap();
        assert_eq!(grid.entity_room(e1), Some(target));
    }

    #[test]
    fn move_entity_diagonal() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let start = cell_to_entity_id(5, 5);
        let target = cell_to_entity_id(6, 6); // diagonal neighbor

        grid.place_entity(e1, start).unwrap();
        grid.move_entity(e1, target).unwrap();
        assert_eq!(grid.entity_room(e1), Some(target));
    }

    #[test]
    fn move_entity_too_far_fails() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let start = cell_to_entity_id(5, 5);
        let target = cell_to_entity_id(7, 5); // 2 cells away

        grid.place_entity(e1, start).unwrap();
        assert!(grid.move_entity(e1, target).is_err());
    }

    #[test]
    fn move_entity_to_self_fails() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(5, 5);

        grid.place_entity(e1, cell).unwrap();
        assert!(grid.move_entity(e1, cell).is_err());
    }

    #[test]
    fn move_entity_out_of_bounds_fails() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let start = cell_to_entity_id(9, 9);
        let target = cell_to_entity_id(10, 9); // out of bounds

        grid.place_entity(e1, start).unwrap();
        assert!(grid.move_entity(e1, target).is_err());
    }

    // --- move_to ---

    #[test]
    fn move_to_adjacent() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(5, 5);
        grid.place_entity(e1, cell).unwrap();

        grid.move_to(e1, 6, 5).unwrap();
        assert_eq!(grid.get_position(e1), Some(GridPos::new(6, 5)));
    }

    // --- set_position (teleport) ---

    #[test]
    fn set_position_teleport() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let cell = cell_to_entity_id(0, 0);
        grid.place_entity(e1, cell).unwrap();

        grid.set_position(e1, 9, 9).unwrap();
        assert_eq!(grid.get_position(e1), Some(GridPos::new(9, 9)));
    }

    #[test]
    fn set_position_on_new_entity() {
        let mut grid = default_grid();
        let e1 = entity(1);

        // set_position can place an entity that isn't yet in the grid
        grid.set_position(e1, 3, 3).unwrap();
        assert_eq!(grid.get_position(e1), Some(GridPos::new(3, 3)));
    }

    #[test]
    fn set_position_out_of_bounds() {
        let mut grid = default_grid();
        let e1 = entity(1);
        assert!(grid.set_position(e1, 100, 100).is_err());
    }

    // --- entities_in_same_area ---

    #[test]
    fn entities_in_same_area_basic() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);
        let e3 = entity(3);
        let cell_a = cell_to_entity_id(5, 5);
        let cell_b = cell_to_entity_id(6, 5);

        grid.place_entity(e1, cell_a).unwrap();
        grid.place_entity(e2, cell_a).unwrap();
        grid.place_entity(e3, cell_b).unwrap();

        let same = grid.entities_in_same_area(e1).unwrap();
        assert_eq!(same, vec![e1, e2]);
    }

    // --- neighbors ---

    #[test]
    fn neighbors_center() {
        let grid = default_grid();
        let cell = cell_to_entity_id(5, 5);
        let neighbors = grid.neighbors(cell).unwrap();
        assert_eq!(neighbors.len(), 8); // 8 directions
    }

    #[test]
    fn neighbors_corner() {
        let grid = default_grid();
        let cell = cell_to_entity_id(0, 0);
        let neighbors = grid.neighbors(cell).unwrap();
        // corner: only 3 neighbors in-bounds
        assert_eq!(neighbors.len(), 3);
    }

    #[test]
    fn neighbors_edge() {
        let grid = default_grid();
        let cell = cell_to_entity_id(0, 5); // left edge, middle
        let neighbors = grid.neighbors(cell).unwrap();
        // edge: 5 neighbors
        assert_eq!(neighbors.len(), 5);
    }

    // --- entities_in_radius ---

    #[test]
    fn entities_in_radius_basic() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);
        let e3 = entity(3);

        grid.set_position(e1, 5, 5).unwrap();
        grid.set_position(e2, 6, 5).unwrap(); // distance 1
        grid.set_position(e3, 8, 5).unwrap(); // distance 3

        let nearby = grid.entities_in_radius(5, 5, 1);
        assert!(nearby.contains(&e1));
        assert!(nearby.contains(&e2));
        assert!(!nearby.contains(&e3));
    }

    #[test]
    fn entities_in_radius_zero() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);

        grid.set_position(e1, 5, 5).unwrap();
        grid.set_position(e2, 6, 5).unwrap();

        let exact = grid.entities_in_radius(5, 5, 0);
        assert_eq!(exact, vec![e1]);
    }

    // --- entity_count ---

    #[test]
    fn entity_count_tracks() {
        let mut grid = default_grid();
        assert_eq!(grid.entity_count(), 0);

        let e1 = entity(1);
        let e2 = entity(2);
        grid.set_position(e1, 0, 0).unwrap();
        grid.set_position(e2, 1, 1).unwrap();
        assert_eq!(grid.entity_count(), 2);

        grid.remove_entity(e1).unwrap();
        assert_eq!(grid.entity_count(), 1);
    }

    // --- all_entity_positions ---

    #[test]
    fn all_entity_positions_returns_map() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);
        grid.set_position(e1, 3, 4).unwrap();
        grid.set_position(e2, 7, 8).unwrap();

        let positions = grid.all_entity_positions();
        assert_eq!(positions.len(), 2);
        assert_eq!(positions[&e1], GridPos::new(3, 4));
        assert_eq!(positions[&e2], GridPos::new(7, 8));
    }

    // --- snapshot ---

    #[test]
    fn snapshot_roundtrip() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);
        grid.set_position(e1, 3, 4).unwrap();
        grid.set_position(e2, 7, 8).unwrap();

        let snap = grid.snapshot_state();
        assert_eq!(snap.entities.len(), 2);

        let mut grid2 = GridSpace::new(GridConfig::default());
        grid2.restore_from_snapshot(snap);

        assert_eq!(grid2.get_position(e1), Some(GridPos::new(3, 4)));
        assert_eq!(grid2.get_position(e2), Some(GridPos::new(7, 8)));
        assert_eq!(grid2.entity_count(), 2);
        assert_eq!(grid2.config().width, 10);
    }

    #[test]
    fn snapshot_bincode_roundtrip() {
        let mut grid = default_grid();
        let e1 = entity(1);
        grid.set_position(e1, 5, 5).unwrap();

        let snap = grid.snapshot_state();
        let bytes = bincode::serialize(&snap).unwrap();
        let decoded: GridSpaceSnapshot = bincode::deserialize(&bytes).unwrap();

        assert_eq!(decoded.entities.len(), 1);
        assert_eq!(decoded.config.width, 10);
    }

    // --- broadcast_targets ---

    #[test]
    fn broadcast_targets_same_as_area() {
        let mut grid = default_grid();
        let e1 = entity(1);
        let e2 = entity(2);
        let cell = cell_to_entity_id(5, 5);
        grid.place_entity(e1, cell).unwrap();
        grid.place_entity(e2, cell).unwrap();

        let targets = grid.broadcast_targets(e1).unwrap();
        let area = grid.entities_in_same_area(e1).unwrap();
        assert_eq!(targets, area);
    }
}
