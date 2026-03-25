// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_is_active() -> u32;
    fn scoop_effect_set_active();
    fn scoop_effect_clear();

    fn scoop_effect_perform_slot_write_u64(op_tag: u32, value: u64);
    fn scoop_effect_perform_slot_read_op_tag() -> u32;
    fn scoop_effect_perform_slot_read_u64() -> u64;
}

#[test]
fn effect_tls_active_flag_set_clear_is_observable() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 初值应为 inactive。
        assert_eq!(scoop_effect_is_active(), 0);

        // set 后可读回。
        scoop_effect_set_active();
        assert_eq!(scoop_effect_is_active(), 1);

        // clear 后恢复初值，且可重复调用保持幂等。
        scoop_effect_clear();
        assert_eq!(scoop_effect_is_active(), 0);
        scoop_effect_clear();
        assert_eq!(scoop_effect_is_active(), 0);

        // unregister 会清空 TLS：active flag 必须回到初值。
        scoop_effect_set_active();
        assert_eq!(scoop_effect_is_active(), 1);
        scoop_thread_unregister();
        assert_eq!(scoop_effect_is_active(), 0);
    }
}

#[test]
fn effect_tls_perform_slot_read_write_is_observable() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // clear 后应回到初值。
        scoop_effect_clear();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);

        // write 后可读回。
        scoop_effect_perform_slot_write_u64(7, 123);
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 7);
        assert_eq!(scoop_effect_perform_slot_read_u64(), 123);

        // clear 会清空 slot。
        scoop_effect_clear();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);

        // unregister 会清空 TLS：slot 必须回到初值。
        scoop_effect_perform_slot_write_u64(1, 42);
        assert_eq!(scoop_effect_perform_slot_read_u64(), 42);
        scoop_thread_unregister();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);
    }
}
