#![no_std]

#[no_mangle]
pub extern "C" fn on_load() -> i32 {
    0
}

/// Always triggers unreachable trap — simulates a panic.
#[no_mangle]
pub extern "C" fn on_tick(_tick_number: u64) -> i32 {
    core::arch::wasm32::unreachable()
}

#[no_mangle]
pub extern "C" fn on_event(_event_id: u32, _payload_ptr: u32, _payload_len: u32) -> i32 {
    0
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
