#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

// --- ABI Version ---

pub const ABI_VERSION_MAJOR: u32 = 1;
pub const ABI_VERSION_MINOR: u32 = 0;

// --- Return Codes ---

pub const RESULT_OK: i32 = 0;
pub const RESULT_ERR_SERIALIZE: i32 = -1;
pub const RESULT_ERR_OUT_OF_BOUNDS: i32 = -2;
pub const RESULT_ERR_UNKNOWN_COMPONENT: i32 = -3;
pub const RESULT_ERR_ENTITY_NOT_FOUND: i32 = -4;

// --- WASM ABI Command ---

/// WASM ABI command format. Uses primitive types only (u64/u32)
/// to avoid depending on engine-internal types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WasmCommand {
    SetComponent {
        entity_id: u64,
        component_id: u32,
        data: Vec<u8>,
    },
    RemoveComponent {
        entity_id: u64,
        component_id: u32,
    },
    EmitEvent {
        event_id: u32,
        payload: Vec<u8>,
    },
    SpawnEntity {
        tag: u64,
    },
    DestroyEntity {
        entity_id: u64,
    },
    MoveEntity {
        entity_id: u64,
        target_room_id: u64,
    },
}

/// Serialize a WasmCommand to postcard bytes.
pub fn serialize_command(cmd: &WasmCommand) -> Result<Vec<u8>, postcard::Error> {
    postcard::to_allocvec(cmd)
}

/// Deserialize a WasmCommand from postcard bytes.
pub fn deserialize_command(bytes: &[u8]) -> Result<WasmCommand, postcard::Error> {
    postcard::from_bytes(bytes)
}

// --- Log Levels ---

pub const LOG_TRACE: u32 = 0;
pub const LOG_DEBUG: u32 = 1;
pub const LOG_INFO: u32 = 2;
pub const LOG_WARN: u32 = 3;
pub const LOG_ERROR: u32 = 4;

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn command_postcard_roundtrip() {
        let commands = [
            WasmCommand::MoveEntity {
                entity_id: 42,
                target_room_id: 100,
            },
            WasmCommand::SetComponent {
                entity_id: 1,
                component_id: 10,
                data: alloc::vec![1, 2, 3],
            },
            WasmCommand::RemoveComponent {
                entity_id: 2,
                component_id: 20,
            },
            WasmCommand::EmitEvent {
                event_id: 5,
                payload: alloc::vec![10, 20, 30],
            },
            WasmCommand::SpawnEntity { tag: 999 },
            WasmCommand::DestroyEntity { entity_id: 7 },
        ];

        for cmd in &commands {
            let bytes = serialize_command(cmd).unwrap();
            let restored = deserialize_command(&bytes).unwrap();
            assert_eq!(cmd, &restored);
        }
    }

    #[test]
    fn abi_version_constants() {
        assert_eq!(ABI_VERSION_MAJOR, 1);
        assert_eq!(ABI_VERSION_MINOR, 0);
    }
}
