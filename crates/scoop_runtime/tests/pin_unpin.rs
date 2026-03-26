// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size: u64,
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

    fn scoop_pin(obj: *mut c_void) -> u32;
    fn scoop_unpin(obj: *mut c_void) -> u32;
}

#[test]
fn pin_unpin_keeps_object_alive_and_enforces_pairing() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 尽量从干净状态开始（避免未来 init 时引入 runtime 分配导致测试不稳定）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

        // 分配一个对象但不写入 shadow stack roots：若不 pin，collect 应回收它。
        let obj = scoop_alloc(header_size + 8);
        assert!(!obj.is_null());
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 1) pin 两次：应要求对称 unpin 两次才真正解 pin。
        assert_eq!(scoop_pin(obj), 1);
        assert_eq!(scoop_pin(obj), 1);

        // pin 期间，即使没有 roots，GC 也必须保活对象（spec §15.10）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 2) unpin 一次：仍处于 pinned 状态，不应被回收。
        assert_eq!(scoop_unpin(obj), 1);
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 3) 第二次 unpin：解除最后一次 pin；此时无 roots，应可被回收。
        assert_eq!(scoop_unpin(obj), 1);
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        // 4) 重复 unpin：必须报错（v0 采用返回 0 的方式固定该错误检查语义）。
        assert_eq!(scoop_unpin(obj), 0);

        scoop_thread_unregister();
    }
}

