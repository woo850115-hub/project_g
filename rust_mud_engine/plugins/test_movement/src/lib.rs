#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use plugin_abi::{WasmCommand, RESULT_OK};

// --- Minimal bump allocator for WASM ---

struct BumpAlloc;
static mut HEAP: [u8; 131072] = [0u8; 131072]; // 128KB
static mut HEAP_POS: usize = 0;

unsafe impl GlobalAlloc for BumpAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let pos = unsafe { HEAP_POS };
        let aligned = (pos + align - 1) & !(align - 1);
        let new_pos = aligned + layout.size();
        if new_pos > 131072 {
            return core::ptr::null_mut();
        }
        unsafe { HEAP_POS = new_pos };
        unsafe { HEAP.as_mut_ptr().add(aligned) }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // bump allocator: no-op dealloc
    }
}

#[global_allocator]
static ALLOC: BumpAlloc = BumpAlloc;

// --- Host function imports ---

extern "C" {
    fn host_emit_command(cmd_ptr: u32, cmd_len: u32) -> i32;
    fn host_get_tick() -> u64;
    fn host_random_seed() -> u64;
}

fn emit_command(cmd: &WasmCommand) -> i32 {
    let bytes = match plugin_abi::serialize_command(cmd) {
        Ok(b) => b,
        Err(_) => return plugin_abi::RESULT_ERR_SERIALIZE,
    };
    unsafe { host_emit_command(bytes.as_ptr() as u32, bytes.len() as u32) }
}

// --- Plugin entry points ---

#[no_mangle]
pub extern "C" fn on_load() -> i32 {
    RESULT_OK
}

/// Every 3 ticks, emit a MoveEntity command.
#[no_mangle]
pub extern "C" fn on_tick(_tick_number: u64) -> i32 {
    let tick = unsafe { host_get_tick() };
    let seed = unsafe { host_random_seed() };

    if tick % 3 != 0 {
        return RESULT_OK;
    }

    let entity_id = (seed % 100).wrapping_add(1);
    let target_room = 1000 + (seed % 5);

    let cmd = WasmCommand::MoveEntity {
        entity_id,
        target_room_id: target_room,
    };
    emit_command(&cmd);

    RESULT_OK
}

#[no_mangle]
pub extern "C" fn on_event(_event_id: u32, _payload_ptr: u32, _payload_len: u32) -> i32 {
    RESULT_OK
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
