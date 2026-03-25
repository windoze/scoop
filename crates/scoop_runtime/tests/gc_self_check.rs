// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_gc_self_check() -> u32;
}

#[test]
fn gc_data_structures_self_check_passes() {
    unsafe {
        scoop_runtime_init();
        assert_eq!(scoop_gc_self_check(), 1);
    }
}

