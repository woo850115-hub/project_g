use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::allocator::EntityAllocator;
use crate::error::EcsError;
use crate::types::EntityId;

/// Maps between our stable EntityId and bevy's internal Entity.
#[derive(Debug, Default)]
struct EntityMapping {
    to_bevy: HashMap<EntityId, Entity>,
    from_bevy: HashMap<Entity, EntityId>,
}

impl EntityMapping {
    fn insert(&mut self, eid: EntityId, bevy: Entity) {
        self.to_bevy.insert(eid, bevy);
        self.from_bevy.insert(bevy, eid);
    }

    fn remove_by_eid(&mut self, eid: &EntityId) -> Option<Entity> {
        if let Some(bevy) = self.to_bevy.remove(eid) {
            self.from_bevy.remove(&bevy);
            Some(bevy)
        } else {
            None
        }
    }

    fn get_bevy(&self, eid: &EntityId) -> Option<Entity> {
        self.to_bevy.get(eid).copied()
    }
}

/// Public ECS adapter that hides bevy_ecs internals.
pub struct EcsAdapter {
    world: World,
    mapping: EntityMapping,
    allocator: EntityAllocator,
}

impl EcsAdapter {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            mapping: EntityMapping::default(),
            allocator: EntityAllocator::new(),
        }
    }

    pub fn allocator(&self) -> &EntityAllocator {
        &self.allocator
    }

    pub fn allocator_mut(&mut self) -> &mut EntityAllocator {
        &mut self.allocator
    }

    /// Spawn a new entity, returning a stable EntityId.
    pub fn spawn_entity(&mut self) -> EntityId {
        let eid = self.allocator.allocate();
        let bevy_entity = self.world.spawn_empty().id();
        self.mapping.insert(eid, bevy_entity);
        eid
    }

    /// Spawn an entity with a specific EntityId (for snapshot restore).
    /// The allocator must already track this entity as alive.
    pub fn spawn_entity_with_id(&mut self, eid: EntityId) -> Result<(), EcsError> {
        if self.mapping.get_bevy(&eid).is_some() {
            return Err(EcsError::EntityAlreadyDead(eid)); // reusing error: entity already exists
        }
        let bevy_entity = self.world.spawn_empty().id();
        self.mapping.insert(eid, bevy_entity);
        Ok(())
    }

    /// Despawn an entity.
    pub fn despawn_entity(&mut self, eid: EntityId) -> Result<(), EcsError> {
        if !self.allocator.is_alive(eid) {
            return Err(EcsError::EntityNotFound(eid));
        }
        let bevy_entity = self
            .mapping
            .remove_by_eid(&eid)
            .ok_or(EcsError::EntityNotFound(eid))?;
        // bevy_ecs 0.15: despawn() no longer takes a bool for recursive despawn
        self.world.despawn(bevy_entity);
        self.allocator.deallocate(eid);
        Ok(())
    }

    /// Get a component reference for an entity.
    pub fn get_component<C: Component>(&self, eid: EntityId) -> Result<&C, EcsError> {
        let bevy_entity = self
            .mapping
            .get_bevy(&eid)
            .ok_or(EcsError::EntityNotFound(eid))?;
        self.world
            .entity(bevy_entity)
            .get::<C>()
            .ok_or(EcsError::ComponentNotFound(eid))
    }

    /// Set (insert or overwrite) a component on an entity.
    pub fn set_component<C: Component>(&mut self, eid: EntityId, component: C) -> Result<(), EcsError> {
        let bevy_entity = self
            .mapping
            .get_bevy(&eid)
            .ok_or(EcsError::EntityNotFound(eid))?;
        self.world.entity_mut(bevy_entity).insert(component);
        Ok(())
    }

    /// Remove a component from an entity.
    pub fn remove_component<C: Component>(&mut self, eid: EntityId) -> Result<(), EcsError> {
        let bevy_entity = self
            .mapping
            .get_bevy(&eid)
            .ok_or(EcsError::EntityNotFound(eid))?;
        self.world.entity_mut(bevy_entity).remove::<C>();
        Ok(())
    }

    /// Check if an entity has a specific component.
    pub fn has_component<C: Component>(&self, eid: EntityId) -> bool {
        self.mapping
            .get_bevy(&eid)
            .map(|bevy_entity| self.world.entity(bevy_entity).contains::<C>())
            .unwrap_or(false)
    }

    /// Collect all alive EntityIds that have a specific component.
    pub fn entities_with<C: Component>(&self) -> Vec<EntityId> {
        let mut result = Vec::new();
        // Iterate our mapping and check which ones have the component
        for (&eid, &bevy_entity) in &self.mapping.to_bevy {
            if self.world.entity(bevy_entity).contains::<C>() {
                result.push(eid);
            }
        }
        result.sort();
        result
    }

    /// Number of alive entities.
    pub fn entity_count(&self) -> usize {
        self.allocator.alive_count()
    }

    /// Get all alive entity IDs (sorted for determinism).
    pub fn all_entities(&self) -> Vec<EntityId> {
        let mut ids: Vec<EntityId> = self.mapping.to_bevy.keys().copied().collect();
        ids.sort();
        ids
    }
}

impl Default for EcsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component, Debug, PartialEq)]
    struct Health(pub i32);

    #[test]
    fn spawn_and_despawn() {
        let mut ecs = EcsAdapter::new();
        let e1 = ecs.spawn_entity();
        let e2 = ecs.spawn_entity();
        assert_eq!(ecs.entity_count(), 2);

        ecs.despawn_entity(e1).unwrap();
        assert_eq!(ecs.entity_count(), 1);
        assert!(ecs.despawn_entity(e1).is_err());

        ecs.despawn_entity(e2).unwrap();
        assert_eq!(ecs.entity_count(), 0);
    }

    #[test]
    fn component_crud() {
        let mut ecs = EcsAdapter::new();
        let e = ecs.spawn_entity();

        ecs.set_component(e, Health(100)).unwrap();
        assert_eq!(ecs.get_component::<Health>(e).unwrap().0, 100);

        ecs.set_component(e, Health(50)).unwrap();
        assert_eq!(ecs.get_component::<Health>(e).unwrap().0, 50);

        ecs.remove_component::<Health>(e).unwrap();
        assert!(ecs.get_component::<Health>(e).is_err());
    }

    #[test]
    fn entities_with_filter() {
        let mut ecs = EcsAdapter::new();
        let e1 = ecs.spawn_entity();
        let e2 = ecs.spawn_entity();
        let _e3 = ecs.spawn_entity();

        ecs.set_component(e1, Health(100)).unwrap();
        ecs.set_component(e2, Health(50)).unwrap();

        let with_health = ecs.entities_with::<Health>();
        assert_eq!(with_health.len(), 2);
        assert!(with_health.contains(&e1));
        assert!(with_health.contains(&e2));
    }
}
