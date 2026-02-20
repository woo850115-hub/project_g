use ecs_adapter::{Component, EcsAdapter, EntityId};
use persistence::error::PersistenceError;
use persistence::registry::{PersistenceRegistry, PersistentComponent};
use serde::{de::DeserializeOwned, Serialize};

use crate::components::*;

/// Generic handler for any Component that implements Serialize + DeserializeOwned.
struct ComponentHandler<C> {
    tag: &'static str,
    _marker: std::marker::PhantomData<C>,
}

impl<C> ComponentHandler<C> {
    fn new(tag: &'static str) -> Self {
        Self {
            tag,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<C> PersistentComponent for ComponentHandler<C>
where
    C: Component + Serialize + DeserializeOwned + Send + Sync,
{
    fn tag(&self) -> &str {
        self.tag
    }

    fn capture(&self, ecs: &EcsAdapter, eid: EntityId) -> Option<Vec<u8>> {
        ecs.get_component::<C>(eid)
            .ok()
            .and_then(|c| bincode::serialize(c).ok())
    }

    fn restore(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        data: &[u8],
    ) -> Result<(), PersistenceError> {
        let c: C = bincode::deserialize(data)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
        ecs.set_component(eid, c)
            .map_err(|e| PersistenceError::Corrupt(e.to_string()))
    }
}

fn register<C>(registry: &mut PersistenceRegistry, tag: &'static str)
where
    C: Component + Serialize + DeserializeOwned + Send + Sync,
{
    registry.register(Box::new(ComponentHandler::<C>::new(tag)));
}

/// Register all MUD component types with the persistence registry.
pub fn register_mud_components(registry: &mut PersistenceRegistry) {
    register::<Name>(registry, "Name");
    register::<Description>(registry, "Description");
    register::<Health>(registry, "Health");
    register::<Attack>(registry, "Attack");
    register::<Defense>(registry, "Defense");
    register::<Inventory>(registry, "Inventory");
    register::<PlayerTag>(registry, "PlayerTag");
    register::<NpcTag>(registry, "NpcTag");
    register::<ItemTag>(registry, "ItemTag");
    register::<InRoom>(registry, "InRoom");
    register::<CombatTarget>(registry, "CombatTarget");
    register::<Dead>(registry, "Dead");
}
