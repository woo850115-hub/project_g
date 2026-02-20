use ecs_adapter::{EcsAdapter, EntityId};

use crate::error::PersistenceError;

/// A trait for components that can be persisted via the snapshot system.
/// Each implementation knows how to capture and restore one component type.
pub trait PersistentComponent: Send + Sync {
    /// Unique tag identifying this component type in snapshots.
    fn tag(&self) -> &str;

    /// Serialize the component from the given entity, if present.
    /// Returns None if the entity does not have this component.
    fn capture(&self, ecs: &EcsAdapter, eid: EntityId) -> Option<Vec<u8>>;

    /// Deserialize and attach the component to the given entity.
    fn restore(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        data: &[u8],
    ) -> Result<(), PersistenceError>;
}

/// Registry of all component types that participate in snapshots.
pub struct PersistenceRegistry {
    components: Vec<Box<dyn PersistentComponent>>,
}

impl PersistenceRegistry {
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
        }
    }

    /// Register a persistent component handler.
    pub fn register(&mut self, comp: Box<dyn PersistentComponent>) {
        self.components.push(comp);
    }

    /// Iterate over all registered component handlers.
    pub fn components(&self) -> &[Box<dyn PersistentComponent>] {
        &self.components
    }
}

impl Default for PersistenceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
