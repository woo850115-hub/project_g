use serde::{de::DeserializeOwned, Serialize};

use crate::error::PluginError;

/// Abstraction over serialization format for WASM ABI.
/// Allows swapping postcard for FlatBuffers in Phase 3 if needed.
pub trait WasmSerializer: Send + Sync {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, PluginError>;
    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, PluginError>;
}

/// postcard-based serializer (Phase 1 default).
#[derive(Debug, Clone, Default)]
pub struct PostcardSerializer;

impl WasmSerializer for PostcardSerializer {
    fn serialize<T: Serialize>(&self, value: &T) -> Result<Vec<u8>, PluginError> {
        postcard::to_allocvec(value)
            .map_err(|e| PluginError::SerializationError(e.to_string()))
    }

    fn deserialize<T: DeserializeOwned>(&self, bytes: &[u8]) -> Result<T, PluginError> {
        postcard::from_bytes(bytes)
            .map_err(|e| PluginError::SerializationError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plugin_abi::WasmCommand;

    #[test]
    fn postcard_roundtrip() {
        let serializer = PostcardSerializer;
        let cmd = WasmCommand::MoveEntity {
            entity_id: 42,
            target_room_id: 100,
        };
        let bytes = serializer.serialize(&cmd).unwrap();
        let restored: WasmCommand = serializer.deserialize(&bytes).unwrap();
        assert_eq!(cmd, restored);
    }

    #[test]
    fn postcard_invalid_bytes() {
        let serializer = PostcardSerializer;
        let result: Result<WasmCommand, _> = serializer.deserialize(&[0xFF, 0xFF, 0xFF]);
        assert!(result.is_err());
    }
}
