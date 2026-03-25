// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_is_registered() -> u32;
    fn scoop_thread_register();
    fn scoop_thread_unregister();
}

#[test]
fn thread_register_unregister_is_callable_and_idempotent() {
    unsafe {
        scoop_runtime_init();

        // 未注册时 unregister 不应崩溃。
        scoop_thread_unregister();
        assert_eq!(scoop_thread_is_registered(), 0);

        // register 可重复调用，且应保持 registered=1。
        scoop_thread_register();
        assert_eq!(scoop_thread_is_registered(), 1);
        scoop_thread_register();
        assert_eq!(scoop_thread_is_registered(), 1);

        // unregister 可重复调用，且应恢复 registered=0。
        scoop_thread_unregister();
        assert_eq!(scoop_thread_is_registered(), 0);
        scoop_thread_unregister();
        assert_eq!(scoop_thread_is_registered(), 0);
    }
}

