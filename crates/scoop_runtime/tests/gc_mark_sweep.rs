// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use core::ffi::c_void;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut c_void;

    fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
    fn scoop_leave_native();

    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend（gc-minimal/gc-hosted）不支持 native_roots（该测试依赖 enter_native roots slots）"
)]
fn gc_collect_mark_sweep_keeps_rooted_objects_and_frees_garbage() {
    assert!(
        std::hint::black_box(GC_CAPABILITIES.native_roots),
        "该测试依赖 native_roots（enter_native roots slots）；当前 backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}"
    );

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

        // 3) Rust 测试代码不产生 statepoint stackmaps；因此用 enter_native 注册 roots slots。
        let mut keep_slot = keep;
        let root0: *mut *mut c_void = &mut keep_slot;
        let mut roots: [*mut *mut c_void; 1] = [root0];
        scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

        // 4) collect：应回收垃圾对象，仅保留 keep。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 5) 离开 InNative 后再 collect：keep 也应被回收。
        scoop_leave_native();
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        scoop_thread_unregister();
    }
}
