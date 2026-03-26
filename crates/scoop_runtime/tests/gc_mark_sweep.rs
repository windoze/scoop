// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::ptr;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size: u64,
    flags: u32,
    mark: u32,
}

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopGcFrame`（root_count=1 的固定版本）。
#[repr(C)]
struct ScoopGcFrame {
    prev: *mut ScoopGcFrame,
    root_count: u32,
    _reserved_u32: u32,
    roots: [*mut c_void; 1],
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut c_void;

    fn scoop_gc_frame_push(frame: *mut ScoopGcFrame);
    fn scoop_gc_frame_pop(frame: *mut ScoopGcFrame);

    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;
}

#[test]
fn gc_collect_mark_sweep_keeps_rooted_objects_and_frees_garbage() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 确保起始为干净状态（即便未来在 init 时引入 runtime 分配，这里也能自洽）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

        // 1) 分配 1 个“将被 roots 持有”的对象。
        let keep = scoop_alloc(header_size + 8);
        assert!(!keep.is_null());

        // 2) 分配若干垃圾对象（不进入 roots）。
        for _ in 0..10 {
            let p = scoop_alloc(header_size + 8);
            assert!(!p.is_null());
        }

        assert_eq!(scoop_gc_debug_heap_object_count(), 11);

        // 3) 构造 shadow stack frame，把 keep 写入 roots 并 push。
        let mut frame = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [keep],
        };
        scoop_gc_frame_push(&mut frame);

        // 4) collect：应回收垃圾对象，仅保留 keep。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 5) pop roots 后再 collect：keep 也应被回收。
        scoop_gc_frame_pop(&mut frame);
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        scoop_thread_unregister();
    }
}
