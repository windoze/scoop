// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_runtime_is_initialized() -> u32;
    fn scoop_runtime_init_count() -> u32;
}

#[test]
fn runtime_init_is_callable_and_observable() {
    unsafe {
        let before_count = scoop_runtime_init_count();

        scoop_runtime_init();
        assert_eq!(scoop_runtime_is_initialized(), 1);
        assert_eq!(scoop_runtime_init_count(), before_count.saturating_add(1));

        scoop_runtime_init();
        assert_eq!(scoop_runtime_is_initialized(), 1);
        assert_eq!(
            scoop_runtime_init_count(),
            before_count.saturating_add(2)
        );
    }
}
