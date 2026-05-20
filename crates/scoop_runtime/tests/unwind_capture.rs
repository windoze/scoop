#[link(name = "scooprt_test_core", kind = "static")]
unsafe extern "C" {
    fn scoop_test_unwind_capture_ips(out_ips: *mut usize, out_cap: u32, skip_frames: u32) -> u32;
    fn scoop_test_unwind_dump_frames_and_stackmap_hits() -> isize;
}

#[test]
fn unwind_capture_ips_smoke() {
    let mut ips = vec![0usize; 64];
    let n =
        unsafe { scoop_test_unwind_capture_ips(ips.as_mut_ptr(), ips.len() as u32, 0) } as usize;

    assert!(n > 0, "expected at least 1 frame, got {n}");
    assert!(n <= ips.len());
    assert!(
        ips[..n].iter().any(|&ip| ip != 0),
        "expected at least one non-zero instruction pointer"
    );
}

#[test]
fn unwind_dump_frames_and_stackmap_hits_smoke() {
    let visited = unsafe { scoop_test_unwind_dump_frames_and_stackmap_hits() };

    // Windows backend 目前仅保证 x86_64 的 ctx walk（用于后续 stackmap roots 枚举/更新）。
    if cfg!(target_os = "windows") && !cfg!(target_arch = "x86_64") {
        assert_eq!(visited, 0);
        return;
    }

    assert!(visited > 0, "expected at least 1 frame, got {visited}");
}
