// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopGcFrame`（TODO T0905）。
//
// 说明：
// - 该结构体在真实场景下将由编译器插桩在函数栈上分配；
// - 本测试只验证 push/pop 维护 TLS 链头的语义，不涉及 root 扫描。
#[repr(C)]
struct ScoopGcFrame {
    prev: *mut ScoopGcFrame,
    root_count: u32,
    _reserved_u32: u32,
    roots: [*mut c_void; 1],
}

#[repr(C)]
struct ScoopGcFrame2 {
    prev: *mut ScoopGcFrame,
    root_count: u32,
    _reserved_u32: u32,
    roots: [*mut c_void; 2],
}

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_gc_current_frame() -> *mut ScoopGcFrame;
    fn scoop_gc_frame_push(frame: *mut ScoopGcFrame);
    fn scoop_gc_frame_pop(frame: *mut ScoopGcFrame);

    fn scoop_gc_shadow_stack_visit_roots_current_thread(
        visitor: extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void),
        ctx: *mut c_void,
    ) -> u64;

    fn scoop_gc_debug_count_roots_current_thread() -> u64;
}

// roots 扫描 visitor：把每个被访问到的 root slot 计数 +1。
//
// 说明：
// - 该函数会被 C runtime 回调，因此必须是 `extern "C"`；
// - 测试用例里只做计数，不解引用 slot，避免引入 UB 风险。
extern "C" fn count_root_slot(_slot: *mut *mut c_void, ctx: *mut c_void) {
    unsafe {
        let count = &mut *(ctx as *mut u64);
        *count += 1;
    }
}

#[test]
fn shadow_stack_push_pop_two_frames_rewinds_current_frame() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        assert_eq!(scoop_gc_current_frame(), ptr::null_mut());

        let mut frame1 = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [ptr::null_mut()],
        };

        scoop_gc_frame_push(&mut frame1);
        assert_eq!(scoop_gc_current_frame(), &mut frame1 as *mut _);

        let mut frame2 = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [ptr::null_mut()],
        };

        scoop_gc_frame_push(&mut frame2);
        assert_eq!(scoop_gc_current_frame(), &mut frame2 as *mut _);
        assert_eq!(frame2.prev, &mut frame1 as *mut _);

        scoop_gc_frame_pop(&mut frame2);
        assert_eq!(scoop_gc_current_frame(), &mut frame1 as *mut _);

        scoop_gc_frame_pop(&mut frame1);
        assert_eq!(scoop_gc_current_frame(), ptr::null_mut());

        // unregister 会清空 TLS（即便 shadow stack 已为空，也应保持幂等）。
        scoop_thread_unregister();
        assert_eq!(scoop_gc_current_frame(), ptr::null_mut());
    }
}

#[test]
fn shadow_stack_debug_count_roots_counts_non_null_slots() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let mut a = 1u8;
        let mut b = 2u8;

        let mut frame1 = ScoopGcFrame2 {
            prev: ptr::null_mut(),
            root_count: 2,
            _reserved_u32: 0,
            roots: [ptr::null_mut(), (&mut a as *mut u8).cast::<c_void>()],
        };

        scoop_gc_frame_push((&mut frame1 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

        let mut frame2 = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [(&mut b as *mut u8).cast::<c_void>()],
        };

        scoop_gc_frame_push(&mut frame2);

        // frame2 有 1 个非空 root，frame1 有 1 个非空 root，总计 2。
        assert_eq!(scoop_gc_debug_count_roots_current_thread(), 2);

        scoop_gc_frame_pop(&mut frame2);
        scoop_gc_frame_pop((&mut frame1 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

        scoop_thread_unregister();
    }
}

#[test]
fn shadow_stack_scan_roots_two_frames_collects_all_roots() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let mut a = 1u8;
        let mut b = 2u8;
        let mut c = 3u8;
        let mut d = 4u8;

        let mut frame1 = ScoopGcFrame2 {
            prev: ptr::null_mut(),
            root_count: 2,
            _reserved_u32: 0,
            roots: [
                (&mut a as *mut u8).cast::<c_void>(),
                (&mut b as *mut u8).cast::<c_void>(),
            ],
        };

        scoop_gc_frame_push((&mut frame1 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

        let mut frame2 = ScoopGcFrame2 {
            prev: ptr::null_mut(),
            root_count: 2,
            _reserved_u32: 0,
            roots: [
                (&mut c as *mut u8).cast::<c_void>(),
                (&mut d as *mut u8).cast::<c_void>(),
            ],
        };

        scoop_gc_frame_push((&mut frame2 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

        let mut visited_by_visitor = 0u64;
        let visited = scoop_gc_shadow_stack_visit_roots_current_thread(
            count_root_slot,
            (&mut visited_by_visitor as *mut u64).cast::<c_void>(),
        );

        // frame2 与 frame1 各有 2 个非空 root，总计 4。
        assert_eq!(visited, 4);
        assert_eq!(visited_by_visitor, 4);

        scoop_gc_frame_pop((&mut frame2 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());
        scoop_gc_frame_pop((&mut frame1 as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

        scoop_thread_unregister();
    }
}
