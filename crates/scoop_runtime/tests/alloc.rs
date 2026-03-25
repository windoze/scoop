// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_alloc(size: u64) -> *mut c_void;

    fn free(ptr: *mut c_void);
}

#[test]
fn scoop_alloc_returns_non_null_and_can_be_called_repeatedly() {
    unsafe {
        // 目前 `scoop_alloc` 还不依赖运行时状态，但显式 init 能让未来引入 GC/TLS 时更稳健。
        scoop_runtime_init();

        let p1 = scoop_alloc(16);
        assert!(!p1.is_null(), "scoop_alloc(16) must return non-null");
        *(p1 as *mut u8) = 0xAB;
        free(p1);

        let p2 = scoop_alloc(16);
        assert!(!p2.is_null(), "scoop_alloc(16) must be callable repeatedly");
        *(p2 as *mut u8) = 0xCD;
        free(p2);
    }
}

