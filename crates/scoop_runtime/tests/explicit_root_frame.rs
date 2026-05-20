#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    #[cfg(not(any(feature = "gc-minimal", feature = "gc-hosted")))]
    fn scoop_test_explicit_root_frame_enter_native_smoke() -> isize;
    fn scoop_test_explicit_root_frame_top() -> usize;
    fn scoop_test_explicit_root_frame_root_map_smoke() -> isize;
}

#[test]
fn explicit_root_frame_tls_top_and_descriptor_walk_smoke() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        assert_eq!(
            scoop_test_explicit_root_frame_top(),
            0,
            "freshly registered thread should start with an empty explicit root frame chain"
        );

        let rc = scoop_test_explicit_root_frame_root_map_smoke();
        assert_eq!(rc, 1, "explicit root frame smoke failed with code {rc}");

        assert_eq!(
            scoop_test_explicit_root_frame_top(),
            0,
            "smoke helper must restore explicit root frame TLS top"
        );

        scoop_thread_unregister();

        assert_eq!(
            scoop_test_explicit_root_frame_top(),
            0,
            "thread unregister must clear explicit root frame TLS top"
        );
    }
}

#[cfg(not(any(feature = "gc-minimal", feature = "gc-hosted")))]
#[test]
fn explicit_root_frame_enter_native_uses_saved_tls_chain() {
    unsafe {
        let rc = scoop_test_explicit_root_frame_enter_native_smoke();
        assert_eq!(rc, 1, "explicit enter_native smoke failed with code {rc}");
    }
}
