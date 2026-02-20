use ecs_adapter::EntityId;

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("entity {0} not found in any room")]
    EntityNotInRoom(EntityId),

    #[error("target room {0} does not exist")]
    RoomNotFound(EntityId),

    #[error("no exit from room {from} to room {to}")]
    NoExit { from: EntityId, to: EntityId },

    #[error("entity {0} already in a room")]
    AlreadyPlaced(EntityId),

    #[error("position ({x}, {y}) is out of bounds")]
    OutOfBounds { x: i32, y: i32 },
}

/// Trait abstracting spatial models (room-based, grid-based, etc.)
pub trait SpaceModel {
    /// All entities in the same area as the given entity.
    fn entities_in_same_area(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError>;

    /// Neighboring room IDs accessible from the entity's current room.
    fn neighbors(&self, room: EntityId) -> Result<Vec<EntityId>, MoveError>;

    /// Move an entity to a target room (must be a neighbor).
    fn move_entity(&mut self, entity: EntityId, target_room: EntityId) -> Result<(), MoveError>;

    /// All entities that should receive broadcasts from the given entity's room.
    fn broadcast_targets(&self, entity: EntityId) -> Result<Vec<EntityId>, MoveError>;

    /// Place an entity in a room (initial placement, no neighbor check).
    fn place_entity(&mut self, entity: EntityId, room: EntityId) -> Result<(), MoveError>;

    /// Remove an entity from its current room.
    fn remove_entity(&mut self, entity: EntityId) -> Result<(), MoveError>;

    /// Get the room an entity is currently in.
    fn entity_room(&self, entity: EntityId) -> Option<EntityId>;
}
