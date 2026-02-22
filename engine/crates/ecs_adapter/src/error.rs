use crate::types::EntityId;

#[derive(Debug, thiserror::Error)]
pub enum EcsError {
    #[error("entity not found: {0}")]
    EntityNotFound(EntityId),

    #[error("entity already dead: {0}")]
    EntityAlreadyDead(EntityId),

    #[error("component not found for entity {0}")]
    ComponentNotFound(EntityId),

    #[error("stale entity reference: {0} (current generation differs)")]
    StaleEntity(EntityId),
}
