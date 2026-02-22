use std::collections::BTreeMap;

use ecs_adapter::{ComponentId, EntityId, EventId};
use serde::{Deserialize, Serialize};

/// Engine commands that modify world state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineCommand {
    SetComponent {
        entity: EntityId,
        component_id: ComponentId,
        data: Vec<u8>,
    },
    RemoveComponent {
        entity: EntityId,
        component_id: ComponentId,
    },
    EmitEvent {
        event_id: EventId,
        payload: Vec<u8>,
    },
    SpawnEntity {
        /// Tag to track which spawn this corresponds to.
        tag: u64,
    },
    DestroyEntity {
        entity: EntityId,
    },
    MoveEntity {
        entity: EntityId,
        target_room: EntityId,
    },
}

/// Deterministic key for LWW conflict resolution on SetComponent/RemoveComponent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ComponentKey {
    entity: EntityId,
    component_id: ComponentId,
}

/// Collects commands during a tick and resolves conflicts deterministically.
#[derive(Debug, Default)]
pub struct CommandStream {
    commands: Vec<EngineCommand>,
}

/// Resolved command list after LWW conflict resolution.
#[derive(Debug)]
pub struct ResolvedCommands {
    pub commands: Vec<EngineCommand>,
}

impl CommandStream {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, cmd: EngineCommand) {
        self.commands.push(cmd);
    }

    /// Resolve conflicts using Last Writer Wins for SetComponent/RemoveComponent
    /// on the same (Entity, ComponentId). Other commands are kept in order.
    /// Final output is sorted by EntityId → ComponentId for determinism.
    pub fn resolve(&self) -> ResolvedCommands {
        // Separate component-keyed commands (LWW) from other commands
        let mut lww_map: BTreeMap<ComponentKey, EngineCommand> = BTreeMap::new();
        let mut other_commands: Vec<EngineCommand> = Vec::new();

        for cmd in &self.commands {
            match cmd {
                EngineCommand::SetComponent {
                    entity,
                    component_id,
                    ..
                }
                | EngineCommand::RemoveComponent {
                    entity,
                    component_id,
                } => {
                    let key = ComponentKey {
                        entity: *entity,
                        component_id: *component_id,
                    };
                    // Last writer wins: later entry overwrites
                    lww_map.insert(key, cmd.clone());
                }
                _ => {
                    other_commands.push(cmd.clone());
                }
            }
        }

        // BTreeMap iteration is sorted by key (EntityId → ComponentId) for determinism
        let mut resolved: Vec<EngineCommand> = lww_map.into_values().collect();
        resolved.extend(other_commands);

        ResolvedCommands { commands: resolved }
    }

    pub fn clear(&mut self) {
        self.commands.clear();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lww_same_entity_component() {
        let mut stream = CommandStream::new();
        let entity = EntityId::new(1, 0);
        let cid = ComponentId(10);

        stream.push(EngineCommand::SetComponent {
            entity,
            component_id: cid,
            data: vec![1, 2, 3],
        });
        stream.push(EngineCommand::SetComponent {
            entity,
            component_id: cid,
            data: vec![4, 5, 6],
        });

        let resolved = stream.resolve();
        // Only 1 SetComponent should remain (the last one)
        let set_cmds: Vec<_> = resolved
            .commands
            .iter()
            .filter(|c| matches!(c, EngineCommand::SetComponent { .. }))
            .collect();
        assert_eq!(set_cmds.len(), 1);
        if let EngineCommand::SetComponent { data, .. } = &set_cmds[0] {
            assert_eq!(data, &vec![4, 5, 6]);
        }
    }

    #[test]
    fn different_entities_no_conflict() {
        let mut stream = CommandStream::new();
        let e1 = EntityId::new(1, 0);
        let e2 = EntityId::new(2, 0);
        let cid = ComponentId(10);

        stream.push(EngineCommand::SetComponent {
            entity: e1,
            component_id: cid,
            data: vec![1],
        });
        stream.push(EngineCommand::SetComponent {
            entity: e2,
            component_id: cid,
            data: vec![2],
        });

        let resolved = stream.resolve();
        let set_cmds: Vec<_> = resolved
            .commands
            .iter()
            .filter(|c| matches!(c, EngineCommand::SetComponent { .. }))
            .collect();
        assert_eq!(set_cmds.len(), 2);
    }

    #[test]
    fn deterministic_ordering() {
        let mut stream = CommandStream::new();
        let e2 = EntityId::new(2, 0);
        let e1 = EntityId::new(1, 0);

        stream.push(EngineCommand::SetComponent {
            entity: e2,
            component_id: ComponentId(5),
            data: vec![2],
        });
        stream.push(EngineCommand::SetComponent {
            entity: e1,
            component_id: ComponentId(3),
            data: vec![1],
        });

        let resolved = stream.resolve();
        // Should be sorted: e1 before e2
        if let EngineCommand::SetComponent { entity, .. } = &resolved.commands[0] {
            assert_eq!(*entity, e1);
        }
        if let EngineCommand::SetComponent { entity, .. } = &resolved.commands[1] {
            assert_eq!(*entity, e2);
        }
    }

    #[test]
    fn remove_overwrites_set() {
        let mut stream = CommandStream::new();
        let entity = EntityId::new(1, 0);
        let cid = ComponentId(10);

        stream.push(EngineCommand::SetComponent {
            entity,
            component_id: cid,
            data: vec![1, 2, 3],
        });
        stream.push(EngineCommand::RemoveComponent {
            entity,
            component_id: cid,
        });

        let resolved = stream.resolve();
        let comp_cmds: Vec<_> = resolved
            .commands
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    EngineCommand::SetComponent { .. } | EngineCommand::RemoveComponent { .. }
                )
            })
            .collect();
        assert_eq!(comp_cmds.len(), 1);
        assert!(matches!(comp_cmds[0], EngineCommand::RemoveComponent { .. }));
    }
}
