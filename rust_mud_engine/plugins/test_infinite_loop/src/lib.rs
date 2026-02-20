#![no_std]

#[no_mangle]
pub extern "C" fn on_load() -> i32 {
    0
}

/// Intentionally infinite loop — should be stopped by fuel exhaustion.
#[no_mangle]
pub extern "C" fn on_tick(_tick_number: u64) -> i32 {
    let mut x: u64 = 0;
    loop {
        x = x.wrapping_add(1);
        // black_box prevents the compiler from optimizing away the loop
        core::hint::black_box(x);
    }
}

#[no_mangle]
pub extern "C" fn on_event(_event_id: u32, _payload_ptr: u32, _payload_len: u32) -> i32 {
    0
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
