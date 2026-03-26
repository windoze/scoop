// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）。
#[repr(C)]
struct ScoopEffectHandlerFrame {
    prev: *mut ScoopEffectHandlerFrame,
    op_tag: u32,
    active: u32,
}

// 对齐 `runtime/c/scoop_gc.h` 的对象头（用于在测试中读取 continuation 捕获字段）。
#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size: u64,
    flags: u32,
    mark: u32,
}

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopContinuation` 前缀布局（TODO T0914）。
//
// 说明：
// - 本测试只需要读取 capture 的 handler stack 指针与 resumed 状态位；
// - 因此只声明前缀字段，避免把后续可演进字段（state/step_fn）锁死在测试里。
#[repr(C)]
struct ScoopContinuationPrefix {
    hdr: ScoopGcObjectHeader,
    resumed: u32,
    _reserved_u32: u32,
    captured_handler_stack_top: *mut ScoopEffectHandlerFrame,
}

type ScoopContinuationStepFn = Option<extern "C" fn(state: *mut c_void, resume_value: u64)>;

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_top() -> *mut ScoopEffectHandlerFrame;

    fn scoop_continuation_alloc(state: *mut c_void, step_fn: ScoopContinuationStepFn) -> *mut c_void;
    fn scoop_continuation_try_resume(continuation: *mut c_void) -> u32;
}

extern "C" fn noop_step(_state: *mut c_void, _resume_value: u64) {}

#[test]
fn continuation_alloc_captures_handler_stack_and_is_one_shot() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());

        let mut frame = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        scoop_effect_handler_stack_push(&mut frame, 42);
        let top = scoop_effect_handler_stack_top();
        assert_eq!(top, &mut frame as *mut _);

        let k = scoop_continuation_alloc(ptr::null_mut(), Some(noop_step));
        assert!(!k.is_null(), "scoop_continuation_alloc must return non-null");

        let prefix = &*(k as *const ScoopContinuationPrefix);
        assert_eq!(
            prefix.captured_handler_stack_top, top,
            "continuation must capture handler stack top at suspension point"
        );

        // one-shot：第一次成功，第二次必须失败（spec §5.5：runtime error）。
        assert_eq!(scoop_continuation_try_resume(k), 1);
        assert_eq!(prefix.resumed, 1);
        assert_eq!(scoop_continuation_try_resume(k), 0);

        scoop_effect_handler_stack_pop(&mut frame);
        scoop_thread_unregister();
    }
}

