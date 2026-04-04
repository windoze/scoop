// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_test_unwind_capture_ips(out_ips: *mut usize, out_cap: u32, skip_frames: u32) -> u32;
}

#[test]
fn unwind_capture_ips_smoke() {
    let mut ips = vec![0usize; 64];
    let n =
        unsafe { scoop_test_unwind_capture_ips(ips.as_mut_ptr(), ips.len() as u32, 0) } as usize;

    if cfg!(target_os = "windows") {
        // Windows backend 目前是占位实现：统一返回 0（见 `runtime/c/platform/unwind_win32.c`）。
        assert_eq!(n, 0);
        return;
    }

    assert!(n > 0, "expected at least 1 frame, got {n}");
    assert!(n <= ips.len());
    assert!(
        ips[..n].iter().any(|&ip| ip != 0),
        "expected at least one non-zero instruction pointer"
    );
}
