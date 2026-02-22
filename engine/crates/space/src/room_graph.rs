use std::collections::{HashMap, HashSet};

use ecs_adapter::EntityId;
use serde::{Deserialize, Serialize};

use crate::model::{MoveError, SpaceModel};

/// Exits from a room in cardinal + custom directions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoomExits {
    pub north: Option<EntityId>,
    pub south: Option<EntityId>,
    pub east: Option<EntityId>,
    pub west: Option<EntityId>,
    pub custom: HashMap<String, EntityId>,
}

impl RoomExits {
    pub fn all_exits(&self) -> Vec<EntityId> {
        let mut exits = Vec::new();
        if let Some(id) = self.north {
            exits.push(id);
        }
        if let Some(id) = self.south {
            exits.push(id);
        }
        if let Some(id) = self.east {
            exits.push(id);
        }
        if let Some(id) = self.west {
            exits.push(id);
        }
        for id in self.custom.values() {
            exits.push(*id);
        }
        exits.sort();
        exits.dedup();
        exits
    }
}

/// Room-graph based spatial model.
#[derive(Debug, Default)]
pub struct RoomGraphSpace {
    /// Room ID → set of entities in that room.
    room_occupants: HashMap<EntityId, HashSet<EntityId>>,
    /// Entity → room it's in.
    entity_to_room: HashMap<EntityId, EntityId>,
    /// Room ID → exits.
    room_exits: HashMap<EntityId, RoomExits>,
}

impl RoomGraphSpace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a room with its exits.
    pub fn register_room(&mut self, room_id: EntityId, exits: RoomExits) {
        self.room_occupants.entry(room_id).or_default();
        self.room_exits.insert(room_id, exits);
    }

    /// Check if a room exists.
    pub fn room_exists(&self, room_id: EntityId) -> bool {
        self.room_occupants.contains_key(&room_id)
    }

    /// Get the number of registered rooms.
    pub fn room_count(&self) -> usize {
        self.room_occupants.len()
    }

    /// Get all registered room IDs (sorted).
    pub fn all_rooms(&self) -> Vec<EntityId> {
        let mut rooms: Vec<_> = self.room_occupants.keys().copied().collect();
        rooms.sort();
        rooms
    }

    /// Get the exits for a room.
    pub fn room_exits(&self, room_id: EntityId) -> Option<&RoomExits> {
        self.room_exits.get(&room_id)
    }

    /// Get sorted occupants of a room.
    pub fn room_occupants(&self, room_id: EntityId) -> Vec<EntityId> {
        self.room_occupants
            .get(&room_id)
            .map(|set| {
                let mut v: Vec<_> = set.iter().copied().collect();
                v.sort();
                v
            })
            .unwrap_or_default()
    }

    /// Capture the full space state as a serializable snapshot.
    pub fn snapshot_state(&self) -> SpaceSnapshot {
        let mut rooms = Vec::new();
        let mut all_room_ids: Vec<_> = self.room_occupants.keys().copied().collect();
        all_room_ids.sort();

        for room_id in all_room_ids {
            let exits = self.room_exits.get(&room_id).cloned().unwrap_or_default();
            let mut occupants: Vec<_> = self.room_occupants
                .get(&room_id)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            occupants.sort();
            rooms.push(RoomSnapshot {
                room_id,
                exits,
                occupants,
            });
        }

        SpaceSnapshot { rooms }
    }

    /// Restore space state from a snapshot, replacing all current data.
    pub fn restore_from_snapshot(&mut self, snapshot: SpaceSnapshot) {
        self.room_occupants.clear();
        self.entity_to_room.clear();
        self.room_exits.clear();

        for room_snap in snapshot.rooms {
            let mut occupant_set = HashSet::new();
            for &entity in &room_snap.occupants {
                occupant_set.insert(entity);
                self.entity_to_room.insert(entity, room_snap.room_id);
            }
            self.room_occupants.insert(room_snap.room_id, occupant_set);
            self.room_exits.insert(room_snap.room_id, room_snap.exits);
        }
    }
}

/// Serializable snapshot of a single room.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSnapshot {
    pub room_id: EntityId,
    pub exits: RoomExits,
    pub occupants: Vec<EntityId>,
}

/// Serializable snapshot of the entire space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceSnapshot {
    pub rooms: Vec<RoomSnapshot>,
}

impl SpaceModel for RoomGraphSpace {
    fn entities_in_same_area(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError> {
        let room = self
            .entity_to_room
            .get(&entity)
            .ok_or(MoveError::EntityNotInRoom(entity))?;
        let occupants = self.room_occupants.get(room).cloned().unwrap_or_default();
        let mut result: Vec<_> = occupants.into_iter().collect();
        result.sort();
        Ok(result)
    }

    fn neighbors(&self, room: EntityId) -> Result<Vec<EntityId>, MoveError> {
        let exits = self
            .room_exits
            .get(&room)
            .ok_or(MoveError::RoomNotFound(room))?;
        Ok(exits.all_exits())
    }

    fn move_entity(&mut self, entity: EntityId, target_room: EntityId) -> Result<(), MoveError> {
        let current_room = *self
            .entity_to_room
            .get(&entity)
            .ok_or(MoveError::EntityNotInRoom(entity))?;

        if !self.room_occupants.contains_key(&target_room) {
            return Err(MoveError::RoomNotFound(target_room));
        }

        // Check that target_room is a neighbor of current_room
        let exits = self
            .room_exits
            .get(&current_room)
            .ok_or(MoveError::RoomNotFound(current_room))?;
        if !exits.all_exits().contains(&target_room) {
            return Err(MoveError::NoExit {
                from: current_room,
                to: target_room,
            });
        }

        // Move
        if let Some(occupants) = self.room_occupants.get_mut(&current_room) {
            occupants.remove(&entity);
        }
        self.room_occupants
            .entry(target_room)
            .or_default()
            .insert(entity);
        self.entity_to_room.insert(entity, target_room);

        Ok(())
    }

    fn broadcast_targets(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError> {
        self.entities_in_same_area(entity)
    }

    fn place_entity(&mut self, entity: EntityId, room: EntityId) -> Result<(), MoveError> {
        if self.entity_to_room.contains_key(&entity) {
            return Err(MoveError::AlreadyPlaced(entity));
        }
        if !self.room_occupants.contains_key(&room) {
            return Err(MoveError::RoomNotFound(room));
        }
        self.room_occupants.entry(room).or_default().insert(entity);
        self.entity_to_room.insert(entity, room);
        Ok(())
    }

    fn remove_entity(&mut self, entity: EntityId) -> Result<(), MoveError> {
        let room = self
            .entity_to_room
            .remove(&entity)
            .ok_or(MoveError::EntityNotInRoom(entity))?;
        if let Some(occupants) = self.room_occupants.get_mut(&room) {
            occupants.remove(&entity);
        }
        Ok(())
    }

    fn entity_room(&self, entity: EntityId) -> Option<EntityId> {
        self.entity_to_room.get(&entity).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_two_rooms() -> (RoomGraphSpace, EntityId, EntityId) {
        let mut space = RoomGraphSpace::new();
        let room_a = EntityId::new(100, 0);
        let room_b = EntityId::new(101, 0);

        space.register_room(
            room_a,
            RoomExits {
                north: Some(room_b),
                ..Default::default()
            },
        );
        space.register_room(
            room_b,
            RoomExits {
                south: Some(room_a),
                ..Default::default()
            },
        );

        (space, room_a, room_b)
    }

    #[test]
    fn place_move_remove_cycle() {
        let (mut space, room_a, room_b) = setup_two_rooms();
        let entity = EntityId::new(1, 0);

        // Place
        space.place_entity(entity, room_a).unwrap();
        assert_eq!(space.entity_room(entity), Some(room_a));

        // Same area check
        let same = space.entities_in_same_area(entity).unwrap();
        assert!(same.contains(&entity));

        // Move
        space.move_entity(entity, room_b).unwrap();
        assert_eq!(space.entity_room(entity), Some(room_b));

        // Remove
        space.remove_entity(entity).unwrap();
        assert_eq!(space.entity_room(entity), None);
    }

    #[test]
    fn move_to_nonexistent_room_fails() {
        let (mut space, room_a, _) = setup_two_rooms();
        let entity = EntityId::new(1, 0);
        let fake_room = EntityId::new(999, 0);

        space.place_entity(entity, room_a).unwrap();
        assert!(space.move_entity(entity, fake_room).is_err());
    }

    #[test]
    fn move_to_non_neighbor_fails() {
        let mut space = RoomGraphSpace::new();
        let room_a = EntityId::new(100, 0);
        let room_b = EntityId::new(101, 0);
        let room_c = EntityId::new(102, 0);

        space.register_room(
            room_a,
            RoomExits {
                north: Some(room_b),
                ..Default::default()
            },
        );
        space.register_room(room_b, RoomExits::default());
        space.register_room(room_c, RoomExits::default());

        let entity = EntityId::new(1, 0);
        space.place_entity(entity, room_a).unwrap();

        // room_c is not a neighbor of room_a
        assert!(space.move_entity(entity, room_c).is_err());
    }

    #[test]
    fn double_place_fails() {
        let (mut space, room_a, _) = setup_two_rooms();
        let entity = EntityId::new(1, 0);

        space.place_entity(entity, room_a).unwrap();
        assert!(space.place_entity(entity, room_a).is_err());
    }

    #[test]
    fn neighbors_returns_exits() {
        let (space, room_a, room_b) = setup_two_rooms();
        let neighbors = space.neighbors(room_a).unwrap();
        assert_eq!(neighbors, vec![room_b]);
    }
}
