//! Stress test: WASM memory grow() 10,000 times.

use wasmtime::*;

#[test]
fn memory_grow_10000_times() {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    let memory_type = MemoryType::new(1, None); // Start with 1 page (64KB)
    let memory = Memory::new(&mut store, memory_type).unwrap();

    let test_data = b"hello wasm memory";

    for i in 0..10_000 {
        // Grow by 1 page (64KB)
        let old_pages = memory.grow(&mut store, 1).expect(&format!(
            "memory.grow() failed at iteration {}",
            i
        ));
        assert!(old_pages >= 1);

        // Write to the beginning of the newly grown page
        let offset = old_pages as usize * 65536;
        let data = memory.data_mut(&mut store);
        assert!(
            offset + test_data.len() <= data.len(),
            "write out of bounds at iteration {}",
            i
        );
        data[offset..offset + test_data.len()].copy_from_slice(test_data);

        // Read back and verify (re-acquiring data pointer)
        let data = memory.data(&store);
        let read_back = &data[offset..offset + test_data.len()];
        assert_eq!(
            read_back, test_data,
            "data mismatch at iteration {}",
            i
        );
    }

    // Final size: 1 (initial) + 10,000 = 10,001 pages = ~640MB
    let final_size = memory.data(&store).len();
    assert_eq!(final_size, 10_001 * 65536);
}

#[test]
fn memory_view_safety_after_grow() {
    // Verifies that re-acquiring memory.data() after grow() is safe
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());

    let memory_type = MemoryType::new(1, None);
    let memory = Memory::new(&mut store, memory_type).unwrap();

    // Write to page 0
    {
        let data = memory.data_mut(&mut store);
        data[0..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    // Grow - this may reallocate the underlying buffer
    memory.grow(&mut store, 100).unwrap();

    // Re-acquire and verify old data is still there
    {
        let data = memory.data(&store);
        assert_eq!(&data[0..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    // Write to new pages
    {
        let data = memory.data_mut(&mut store);
        let offset = 100 * 65536; // page 100
        data[offset..offset + 4].copy_from_slice(&[0xCA, 0xFE, 0xBA, 0xBE]);
    }

    // Verify both old and new data
    let data = memory.data(&store);
    assert_eq!(&data[0..4], &[0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(&data[100 * 65536..100 * 65536 + 4], &[0xCA, 0xFE, 0xBA, 0xBE]);
}
