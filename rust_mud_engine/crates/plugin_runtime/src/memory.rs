use wasmtime::{Memory, StoreContextMut};

use crate::error::PluginError;
use crate::host_api::HostState;

/// Safe wrapper around WASM Linear Memory.
///
/// Re-acquires the base pointer on every read/write to handle grow() safely.
/// Holds &mut StoreContextMut so only one view can exist at a time
/// (enforced by Rust's borrow checker).
pub struct WasmMemoryView<'a> {
    memory: Memory,
    store: StoreContextMut<'a, HostState>,
}

impl<'a> WasmMemoryView<'a> {
    pub fn new(memory: Memory, store: StoreContextMut<'a, HostState>) -> Self {
        Self { memory, store }
    }

    /// Read bytes from WASM linear memory. Copies data out.
    pub fn read_bytes(&self, offset: u32, len: u32) -> Result<Vec<u8>, PluginError> {
        let data = self.memory.data(&self.store);
        let start = offset as usize;
        let end = start + len as usize;
        if end > data.len() {
            return Err(PluginError::MemoryOutOfBounds {
                offset,
                len,
                memory_size: data.len(),
            });
        }
        Ok(data[start..end].to_vec())
    }

    /// Write bytes to WASM linear memory.
    pub fn write_bytes(&mut self, offset: u32, bytes: &[u8]) -> Result<(), PluginError> {
        let data = self.memory.data_mut(&mut self.store);
        let start = offset as usize;
        let end = start + bytes.len();
        if end > data.len() {
            return Err(PluginError::MemoryOutOfBounds {
                offset,
                len: bytes.len() as u32,
                memory_size: data.len(),
            });
        }
        data[start..end].copy_from_slice(bytes);
        Ok(())
    }

    /// Current memory size in bytes.
    pub fn size(&self) -> usize {
        self.memory.data(&self.store).len()
    }

    /// Grow memory by `delta_pages` (each page = 64KB).
    pub fn grow(&mut self, delta_pages: u64) -> Result<u64, PluginError> {
        self.memory
            .grow(&mut self.store, delta_pages)
            .map_err(|e| PluginError::LoadError(format!("memory grow failed: {}", e)))
    }
}
