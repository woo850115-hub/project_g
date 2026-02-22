use std::collections::HashMap;

use ecs_adapter::{EcsAdapter, EntityId};
use mlua::Lua;

use crate::error::ScriptError;

/// Trait for bridging a Rust ECS component to/from Lua values.
/// Each concrete type implements this to allow Lua scripts to read/write components.
pub trait ScriptComponent: Send + Sync {
    /// String tag used in Lua scripts (e.g. "Health", "Name").
    fn tag(&self) -> &str;

    /// Read the component from ECS and convert to a Lua value.
    /// Returns None if the entity does not have this component.
    fn get_as_lua(
        &self,
        ecs: &EcsAdapter,
        eid: EntityId,
        lua: &Lua,
    ) -> Result<Option<mlua::Value>, ScriptError>;

    /// Set the component on an entity from a Lua value.
    fn set_from_lua(
        &self,
        ecs: &mut EcsAdapter,
        eid: EntityId,
        value: mlua::Value,
        lua: &Lua,
    ) -> Result<(), ScriptError>;

    /// Check if the entity has this component.
    fn has(&self, ecs: &EcsAdapter, eid: EntityId) -> bool;

    /// Remove this component from the entity.
    fn remove(&self, ecs: &mut EcsAdapter, eid: EntityId) -> Result<(), ScriptError>;

    /// Get all entity IDs that have this component.
    fn entities_with(&self, ecs: &EcsAdapter) -> Vec<EntityId>;
}

/// Registry mapping string tags to ScriptComponent trait objects.
pub struct ScriptComponentRegistry {
    components: HashMap<String, Box<dyn ScriptComponent>>,
}

impl ScriptComponentRegistry {
    pub fn new() -> Self {
        Self {
            components: HashMap::new(),
        }
    }

    /// Register a component handler by its tag.
    pub fn register(&mut self, handler: Box<dyn ScriptComponent>) {
        let tag = handler.tag().to_string();
        self.components.insert(tag, handler);
    }

    /// Look up a handler by tag.
    pub fn get(&self, tag: &str) -> Option<&dyn ScriptComponent> {
        self.components.get(tag).map(|b| b.as_ref())
    }

    /// Get all registered tags (sorted for determinism).
    pub fn tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> = self.components.keys().map(|s| s.as_str()).collect();
        tags.sort();
        tags
    }

    /// Number of registered component types.
    pub fn len(&self) -> usize {
        self.components.len()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl Default for ScriptComponentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_empty() {
        let registry = ScriptComponentRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }
}
