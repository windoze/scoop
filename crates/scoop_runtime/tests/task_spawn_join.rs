// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_task_spawn_int(value: i64) -> u64;
    fn scoop_task_join_int(handle: u64) -> i64;
}

#[test]
fn task_spawn_join_int_roundtrip() {
    unsafe {
        let handle = scoop_task_spawn_int(42);
        assert_ne!(handle, 0);

        let value = scoop_task_join_int(handle);
        assert_eq!(value, 42);
    }
}
