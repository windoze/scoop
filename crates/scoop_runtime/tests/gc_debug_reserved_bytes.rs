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

    fn scoop_pin(obj: *mut c_void) -> u32;
    fn scoop_unpin(obj: *mut c_void) -> u32;

    fn scoop_gc_debug_heap_bytes_allocated() -> u64;
    fn scoop_gc_debug_heap_bytes_freed() -> u64;
    fn scoop_gc_debug_heap_bytes_reserved() -> u64;
}

#[test]
fn debug_reserved_bytes_is_consistent_with_live_bytes() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        // 尽量从干净状态开始：避免其它 runtime 分配影响该断言（以及便于定位回归）。
        scoop_gc_collect();

        let base_alloc = scoop_gc_debug_heap_bytes_allocated();
        let base_freed = scoop_gc_debug_heap_bytes_freed();
        assert!(base_alloc >= base_freed);
        let base_live = base_alloc - base_freed;

        // 分配一个“无 roots 的对象”，并通过 pin 把它固定为额外 root（spec §15.10）。
        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let obj_size = header_size + 256;
        let obj = scoop_alloc(obj_size);
        assert!(!obj.is_null());
        assert_eq!(scoop_pin(obj), 1);

        // GC 后对象仍应存活；且 reserved bytes 至少覆盖 live bytes。
        scoop_gc_collect();

        let allocated = scoop_gc_debug_heap_bytes_allocated();
        let freed = scoop_gc_debug_heap_bytes_freed();
        assert!(allocated >= freed);
        let live = allocated - freed;
        assert_eq!(live, base_live + obj_size);

        let reserved = scoop_gc_debug_heap_bytes_reserved();
        assert!(reserved >= live);

        // backend 语义约定（用于 microbench）：
        // - baseline/minimal：reserved≈live（逐对象 malloc/free）
        // - immix：reserved 至少包含一个 32KiB block（non-moving，稀疏存活会放大该值）
        if cfg!(feature = "gc-immix") {
            assert!(reserved >= 32 * 1024);
            assert!(reserved > live);
        } else {
            assert_eq!(reserved, live);
        }

        assert_eq!(scoop_unpin(obj), 1);
        scoop_gc_collect();

        scoop_thread_unregister();
    }
}

