#[cfg(feature = "gc-immix")]
mod common;

#[cfg(feature = "gc-immix")]
#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_gc_registered_thread_exit_without_unregister() -> isize;
}

#[cfg(feature = "gc-immix")]
#[test]
fn registered_thread_exit_without_unregister_does_not_block_stw() {
    let rc = unsafe { scoop_test_gc_registered_thread_exit_without_unregister() };
    assert_eq!(
        rc,
        1,
        "registered thread exit hook/STW regression failed: backend={}, caps={}, rc={rc}",
        common::gc_backend_name(),
        common::gc_capabilities_debug()
    );
}
