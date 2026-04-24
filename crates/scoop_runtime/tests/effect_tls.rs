// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use std::ffi::c_void;

#[repr(C)]
struct ScoopValueTransport {
    word: u64,
    gc_ref: *mut c_void,
}

#[repr(C)]
struct ScoopEffectSignal {
    op_tag: u32,
    effect_instance_key: u32,
    payload: ScoopValueTransport,
    resume_token: *mut c_void,
}

#[repr(C)]
struct ScoopEffectOutcome {
    tag: u32,
    reserved_u32: u32,
    complete: ScoopValueTransport,
    signal: ScoopEffectSignal,
}

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_is_active() -> u32;
    fn scoop_effect_outcome_consume_current(outcome: *mut ScoopEffectOutcome);
    fn scoop_effect_outcome_publish(outcome: *const ScoopEffectOutcome);
    fn scoop_effect_set_active();
    fn scoop_effect_clear();

    fn scoop_callee_suspend_state_get() -> *mut c_void;
    fn scoop_callee_suspend_state_clear();
    fn scoop_test_callee_suspend_state_set(state: *mut c_void);

    fn scoop_effect_perform_slot_write_u64_with_gc_ref(
        op_tag: u32,
        effect_instance_key: u32,
        value: u64,
        gc_ref: *mut c_void,
    );
    fn scoop_effect_perform_slot_write_u64(op_tag: u32, effect_instance_key: u32, value: u64);
    fn scoop_effect_perform_slot_write_u64_2(
        op_tag: u32,
        effect_instance_key: u32,
        word0: u64,
        word1: u64,
    );
    fn scoop_effect_perform_slot_read_op_tag() -> u32;
    fn scoop_effect_perform_slot_read_effect_instance_key() -> u32;
    fn scoop_effect_perform_slot_read_len_words() -> u32;
    fn scoop_effect_perform_slot_read_gc_ref() -> *mut c_void;
    fn scoop_effect_perform_slot_read_u64() -> u64;
    fn scoop_effect_perform_slot_read_u64_at(index: u32) -> u64;

    fn scoop_sync_once_create() -> *mut c_void;
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
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(1), 0);

        // write 1 word 后可读回，包括 effect_instance_key。
        scoop_effect_perform_slot_write_u64(7, 5, 123);
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 7);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 5);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 1);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_u64(), 123);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 123);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(1), 0);

        // write 2 words：复合 payload（TODO T0630）的最小可观测行为。
        scoop_effect_perform_slot_write_u64_2(9, 6, 11, 22);
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 9);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 6);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 2);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 11);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(1), 22);

        // 越界读取应返回 0（避免引入额外崩溃点）。
        assert_eq!(scoop_effect_perform_slot_read_u64_at(999), 0);

        // clear 会清空 slot（包括 len 与所有 words）。
        scoop_effect_clear();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(1), 0);

        // unregister 会清空 TLS：slot 必须回到初值。
        scoop_effect_perform_slot_write_u64(1, 8, 42);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 42);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 8);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 1);
        scoop_thread_unregister();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_u64(), 0);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 0);
    }
}

#[test]
fn effect_tls_perform_slot_gc_ref_read_write_is_observable() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        scoop_effect_clear();
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);

        let once = scoop_sync_once_create();
        assert!(!once.is_null(), "sync once object must be allocated");

        scoop_effect_perform_slot_write_u64_with_gc_ref(13, 17, 77, once);
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 13);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 17);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 1);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 77);
        assert_eq!(scoop_effect_perform_slot_read_gc_ref(), once);

        scoop_effect_clear();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());

        scoop_effect_perform_slot_write_u64_with_gc_ref(21, 29, 99, once);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 29);
        assert_eq!(scoop_effect_perform_slot_read_gc_ref(), once);
        scoop_thread_unregister();
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());
    }
}

#[test]
fn effect_tls_callee_suspend_state_clear_and_unregister_are_observable() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        scoop_callee_suspend_state_clear();
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "clear should reset callee suspend TLS to null"
        );

        let once = scoop_sync_once_create();
        assert!(!once.is_null(), "sync once object must be allocated");

        scoop_test_callee_suspend_state_set(once);
        assert_eq!(
            scoop_callee_suspend_state_get(),
            once,
            "set should make callee suspend TLS observable"
        );

        scoop_callee_suspend_state_clear();
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "clear should remove the previously stored callee suspend TLS value"
        );

        scoop_test_callee_suspend_state_set(once);
        assert_eq!(scoop_callee_suspend_state_get(), once);
        scoop_thread_unregister();
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "thread unregister should clear any stale callee suspend TLS state"
        );
    }
}

#[test]
fn effect_outcome_roundtrip_consumes_and_republishes_current_tls_signal() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        scoop_effect_clear();

        let once = scoop_sync_once_create();
        assert!(!once.is_null(), "sync once object must be allocated");

        scoop_effect_perform_slot_write_u64_with_gc_ref(31, 47, 99, once);
        scoop_test_callee_suspend_state_set(once);
        scoop_effect_set_active();
        assert_eq!(scoop_effect_is_active(), 1);

        let mut outcome = ScoopEffectOutcome {
            tag: u32::MAX,
            reserved_u32: u32::MAX,
            complete: ScoopValueTransport {
                word: u64::MAX,
                gc_ref: once,
            },
            signal: ScoopEffectSignal {
                op_tag: u32::MAX,
                effect_instance_key: u32::MAX,
                payload: ScoopValueTransport {
                    word: u64::MAX,
                    gc_ref: once,
                },
                resume_token: once,
            },
        };

        scoop_effect_outcome_consume_current(&mut outcome);
        assert_eq!(outcome.tag, 1, "active TLS signal should become Propagate");
        assert_eq!(outcome.signal.op_tag, 31);
        assert_eq!(outcome.signal.effect_instance_key, 47);
        assert_eq!(outcome.signal.payload.word, 99);
        assert_eq!(outcome.signal.payload.gc_ref, once);
        assert_eq!(
            outcome.signal.resume_token, once,
            "consume_current should surface the current ordinary callee scratch state as EffectSignal.resume_token"
        );
        assert_eq!(
            scoop_effect_is_active(),
            0,
            "consume should clear the active flag from TLS"
        );
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "consume_current should clear the callee-resume scratch TLS after lifting it into the explicit outcome"
        );
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());

        scoop_effect_outcome_publish(&outcome);
        assert_eq!(
            scoop_effect_is_active(),
            1,
            "publish should restore the active flag"
        );
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 31);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 47);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 1);
        assert_eq!(scoop_effect_perform_slot_read_u64_at(0), 99);
        assert_eq!(scoop_effect_perform_slot_read_gc_ref(), once);
        assert_eq!(
            scoop_callee_suspend_state_get(),
            once,
            "publishing an explicit outcome back to TLS should restore resume_token scratch so the next outer boundary can lift it again"
        );

        let mut republished = ScoopEffectOutcome {
            tag: u32::MAX,
            reserved_u32: u32::MAX,
            complete: ScoopValueTransport {
                word: u64::MAX,
                gc_ref: once,
            },
            signal: ScoopEffectSignal {
                op_tag: u32::MAX,
                effect_instance_key: u32::MAX,
                payload: ScoopValueTransport {
                    word: u64::MAX,
                    gc_ref: once,
                },
                resume_token: std::ptr::null_mut(),
            },
        };
        scoop_effect_outcome_consume_current(&mut republished);
        assert_eq!(republished.tag, 1);
        assert_eq!(republished.signal.op_tag, 31);
        assert_eq!(republished.signal.effect_instance_key, 47);
        assert_eq!(republished.signal.payload.word, 99);
        assert_eq!(republished.signal.payload.gc_ref, once);
        assert_eq!(
            republished.signal.resume_token, once,
            "republished TLS signal must preserve resume_token across multiple ordinary boundaries"
        );
        assert!(
            scoop_callee_suspend_state_get().is_null(),
            "re-consuming the republished signal should clear the temporary resume-token scratch"
        );

        republished.tag = 0;
        republished.signal.op_tag = 0;
        republished.signal.effect_instance_key = 0;
        republished.signal.payload.word = 0;
        republished.signal.payload.gc_ref = std::ptr::null_mut();
        republished.signal.resume_token = std::ptr::null_mut();
        scoop_effect_outcome_publish(&republished);
        assert_eq!(scoop_effect_is_active(), 0);
        assert_eq!(scoop_effect_perform_slot_read_op_tag(), 0);
        assert_eq!(scoop_effect_perform_slot_read_effect_instance_key(), 0);
        assert_eq!(scoop_effect_perform_slot_read_len_words(), 0);
        assert!(scoop_effect_perform_slot_read_gc_ref().is_null());

        scoop_thread_unregister();
    }
}
