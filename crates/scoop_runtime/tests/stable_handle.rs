// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

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
    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;

    fn scoop_handle_new(obj: *mut c_void) -> u64;
    fn scoop_handle_get(handle: u64) -> *mut c_void;
    fn scoop_handle_drop(handle: u64) -> u32;
}

#[test]
fn stable_handle_keeps_object_alive_and_can_be_dropped() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 尽量从干净状态开始（避免未来 init 时引入 runtime 分配导致测试不稳定）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

        // 分配一个对象但不写入 shadow stack roots：若没有 handle/pin，collect 应回收它。
        let obj = scoop_alloc(header_size + 8);
        assert!(!obj.is_null());
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 1) 创建 handle：handle 值必须非 0。
        let handle = scoop_handle_new(obj);
        assert_ne!(handle, 0);
        assert!(!scoop_handle_get(handle).is_null());

        // 2) handle 期间：即使没有 roots，GC 也必须保活对象（spec §15.10.1）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);
        assert!(!scoop_handle_get(handle).is_null());

        // 3) drop handle：之后应允许对象被回收；且 handle 变为无效（get/drop 不崩溃）。
        assert_eq!(scoop_handle_drop(handle), 1);
        assert!(scoop_handle_get(handle).is_null());
        assert_eq!(scoop_handle_drop(handle), 0);

        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        scoop_thread_unregister();
    }
}
