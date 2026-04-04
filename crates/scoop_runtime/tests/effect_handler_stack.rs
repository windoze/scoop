// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ptr;

// 对齐 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）。
//
// 说明：
// - 该结构体预期由编译器在函数栈上分配，并通过 runtime API push/pop；
// - 本测试只验证 TLS handler stack 的 push/pop、最近匹配查询与 active 开关语义。
#[repr(C)]
struct ScoopEffectHandlerFrame {
    prev: *mut ScoopEffectHandlerFrame,
    op_tag: u32,
    active: u32,
}

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_effect_handler_stack_push(frame: *mut ScoopEffectHandlerFrame, op_tag: u32);
    fn scoop_effect_handler_stack_pop(frame: *mut ScoopEffectHandlerFrame);
    fn scoop_effect_handler_stack_set_active(frame: *mut ScoopEffectHandlerFrame, active: u32);
    fn scoop_effect_handler_stack_top() -> *mut ScoopEffectHandlerFrame;
    fn scoop_effect_handler_stack_find_nearest(op_tag: u32) -> *mut ScoopEffectHandlerFrame;
}

#[test]
fn effect_handler_stack_push_pop_rewinds_top() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());

        let mut frame1 = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        scoop_effect_handler_stack_push(&mut frame1, 7);
        assert_eq!(scoop_effect_handler_stack_top(), &mut frame1 as *mut _);
        assert_eq!(frame1.prev, ptr::null_mut());
        assert_eq!(frame1.op_tag, 7);
        assert_eq!(frame1.active, 1);

        let mut frame2 = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        scoop_effect_handler_stack_push(&mut frame2, 9);
        assert_eq!(scoop_effect_handler_stack_top(), &mut frame2 as *mut _);
        assert_eq!(frame2.prev, &mut frame1 as *mut _);
        assert_eq!(frame2.op_tag, 9);
        assert_eq!(frame2.active, 1);

        scoop_effect_handler_stack_pop(&mut frame2);
        assert_eq!(scoop_effect_handler_stack_top(), &mut frame1 as *mut _);

        scoop_effect_handler_stack_pop(&mut frame1);
        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());

        // unregister 会清空 TLS（即便 stack 已为空，也应保持幂等）。
        scoop_thread_unregister();
        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());
    }
}

#[test]
fn effect_handler_stack_find_nearest_skips_inactive_frames() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 约定：op_tag 只是运行期分发用的数值标签；测试里直接使用手写常量。
        //
        // 构造栈：top -> f3(tag=1) -> f2(tag=2) -> f1(tag=1)
        let mut f1 = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        let mut f2 = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };
        let mut f3 = ScoopEffectHandlerFrame {
            prev: ptr::null_mut(),
            op_tag: 0,
            active: 0,
        };

        scoop_effect_handler_stack_push(&mut f1, 1);
        scoop_effect_handler_stack_push(&mut f2, 2);
        scoop_effect_handler_stack_push(&mut f3, 1);

        assert_eq!(
            scoop_effect_handler_stack_find_nearest(1),
            &mut f3 as *mut _
        );
        assert_eq!(
            scoop_effect_handler_stack_find_nearest(2),
            &mut f2 as *mut _
        );
        assert_eq!(scoop_effect_handler_stack_find_nearest(3), ptr::null_mut());

        // Appendix A.4：arm body 执行时将当前 handler 置为 inactive，应命中外层 handler。
        scoop_effect_handler_stack_set_active(&mut f3, 0);
        assert_eq!(f3.active, 0);
        assert_eq!(
            scoop_effect_handler_stack_find_nearest(1),
            &mut f1 as *mut _
        );

        // 把中间层也置为 inactive：tag=2 应该找不到。
        scoop_effect_handler_stack_set_active(&mut f2, 0);
        assert_eq!(scoop_effect_handler_stack_find_nearest(2), ptr::null_mut());

        // pop 仍必须按栈顺序（active 不影响栈结构）。
        scoop_effect_handler_stack_pop(&mut f3);
        scoop_effect_handler_stack_pop(&mut f2);
        scoop_effect_handler_stack_pop(&mut f1);

        // 即便忘记 pop，unregister 也应清空 TLS（用于测试与调试场景的容错）。
        scoop_effect_handler_stack_push(&mut f1, 1);
        assert_ne!(scoop_effect_handler_stack_top(), ptr::null_mut());
        scoop_thread_unregister();
        assert_eq!(scoop_effect_handler_stack_top(), ptr::null_mut());
    }
}
