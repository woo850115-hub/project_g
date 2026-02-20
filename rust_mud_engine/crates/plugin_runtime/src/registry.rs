use std::collections::HashMap;

use ecs_adapter::{ComponentId, EcsAdapter, EntityId};
use serde::Serialize;

use crate::error::PluginError;
use crate::serializer::{PostcardSerializer, WasmSerializer};

/// Trait for serializing a specific component type from the ECS.
pub trait ComponentSerializer: Send + Sync {
    /// Serialize the component from the ECS for a given entity.
    /// Returns None if the entity doesn't have this component.
    fn serialize_from_ecs(
        &self,
        ecs: &EcsAdapter,
        entity: EntityId,
    ) -> Option<Vec<u8>>;
}

/// Type-erased component serializer for a concrete Component type.
struct TypedComponentSerializer<C> {
    serializer: PostcardSerializer,
    _phantom: std::marker::PhantomData<C>,
}

impl<C> ComponentSerializer for TypedComponentSerializer<C>
where
    C: ecs_adapter::Component + Serialize + 'static,
{
    fn serialize_from_ecs(
        &self,
        ecs: &EcsAdapter,
        entity: EntityId,
    ) -> Option<Vec<u8>> {
        let component = ecs.get_component::<C>(entity).ok()?;
        self.serializer.serialize(component).ok()
    }
}

/// Registry mapping ComponentId to serialization functions.
/// Used by host_get_component to serialize components for WASM plugins.
#[derive(Default)]
pub struct ComponentRegistry {
    serializers: HashMap<ComponentId, Box<dyn ComponentSerializer>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a component type with its ComponentId.
    pub fn register<C>(&mut self, component_id: ComponentId)
    where
        C: ecs_adapter::Component + Serialize + 'static,
    {
        self.serializers.insert(
            component_id,
            Box::new(TypedComponentSerializer::<C> {
                serializer: PostcardSerializer,
                _phantom: std::marker::PhantomData,
            }),
        );
    }

    /// Serialize a component for a given entity using its ComponentId.
    pub fn serialize_component(
        &self,
        ecs: &EcsAdapter,
        entity: EntityId,
        component_id: ComponentId,
    ) -> Result<Vec<u8>, PluginError> {
        let serializer = self
            .serializers
            .get(&component_id)
            .ok_or_else(|| {
                PluginError::SerializationError(format!(
                    "no serializer registered for component {:?}",
                    component_id
                ))
            })?;
        serializer
            .serialize_from_ecs(ecs, entity)
            .ok_or_else(|| {
                PluginError::SerializationError(format!(
                    "entity {:?} does not have component {:?}",
                    entity, component_id
                ))
            })
    }

    pub fn has_component(&self, component_id: ComponentId) -> bool {
        self.serializers.contains_key(&component_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecs_adapter::EcsAdapter;
    use serde::Serialize;

    #[derive(ecs_adapter::Component, Debug, Serialize, serde::Deserialize, PartialEq)]
    struct Health(pub i32);

    #[test]
    fn register_and_serialize() {
        let mut registry = ComponentRegistry::new();
        let health_id = ComponentId(1);
        registry.register::<Health>(health_id);

        let mut ecs = EcsAdapter::new();
        let entity = ecs.spawn_entity();
        ecs.set_component(entity, Health(100)).unwrap();

        let bytes = registry.serialize_component(&ecs, entity, health_id).unwrap();
        let restored: Health = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(restored, Health(100));
    }

    #[test]
    fn missing_component_returns_error() {
        let mut registry = ComponentRegistry::new();
        registry.register::<Health>(ComponentId(1));

        let ecs = EcsAdapter::new();
        let fake_entity = EntityId::new(999, 0);
        assert!(registry.serialize_component(&ecs, fake_entity, ComponentId(1)).is_err());
    }

    #[test]
    fn unregistered_component_id_returns_error() {
        let registry = ComponentRegistry::new();
        let ecs = EcsAdapter::new();
        let fake_entity = EntityId::new(0, 0);
        assert!(registry.serialize_component(&ecs, fake_entity, ComponentId(99)).is_err());
    }
}
