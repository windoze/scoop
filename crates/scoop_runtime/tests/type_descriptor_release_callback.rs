// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
type ScoopTypeTraceFn = Option<
    unsafe extern "C" fn(
        object: *mut c_void,
        visitor: ScoopGcTraceVisitor,
        ctx: *mut c_void,
    ) -> u64,
>;
type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（TODO T0907 + T0920）。
#[repr(C)]
struct ScoopTypeDescriptor {
    abi_version: u32,
    flags: u32,
    size_bytes: u64,
    align_bytes: u64,
    trace_start_offset_bytes: u64,
    trace_bitmap_u64_len: u32,
    _reserved_u32: u32,
    trace_bitmap: *const u64,
    trace_fn: ScoopTypeTraceFn,
    release_fn: ScoopTypeReleaseFn,
    type_id: u64,
    parent_type_desc: *const ScoopTypeDescriptor,
    itable: *const c_void,
    vtable: *const c_void,
}

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const ScoopTypeDescriptor,
    size_bytes: u64,
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

static RELEASE_CALLS: AtomicUsize = AtomicUsize::new(0);
static LAST_RELEASED: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn test_release(object: *mut c_void) {
    LAST_RELEASED.store(object as usize, Ordering::SeqCst);
    RELEASE_CALLS.fetch_add(1, Ordering::SeqCst);
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
fn type_descriptor_release_callback_runs_once_on_sweep() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        RELEASE_CALLS.store(0, Ordering::SeqCst);
        LAST_RELEASED.store(0, Ordering::SeqCst);

        // 确保起始为干净状态（即便未来在 init 时引入 runtime 分配，这里也能自洽）。
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);

        // 1) 构造一个带 release callback 的 type descriptor（测试专用）。
        let header_size = mem::size_of::<ScoopGcObjectHeader>() as u64;
        let obj_size = header_size + 8;
        let desc = ScoopTypeDescriptor {
            abi_version: 0,
            flags: 0,
            size_bytes: obj_size,
            align_bytes: mem::align_of::<usize>() as u64,
            trace_start_offset_bytes: 0,
            trace_bitmap_u64_len: 0,
            _reserved_u32: 0,
            trace_bitmap: ptr::null(),
            trace_fn: None,
            release_fn: Some(test_release),
            type_id: 0,
            parent_type_desc: ptr::null(),
            itable: ptr::null(),
            vtable: ptr::null(),
        };

        // 2) 分配 1 个对象，并把 type_desc 指向上述 descriptor。
        let obj = scoop_alloc(obj_size);
        assert!(!obj.is_null());
        let hdr = obj.cast::<ScoopGcObjectHeader>();
        (*hdr).type_desc = &desc;

        assert_eq!(scoop_gc_debug_heap_object_count(), 1);

        // 3) 把对象写入 shadow stack roots：collect 后不应触发 release callback。
        let mut frame = ScoopGcFrame {
            prev: ptr::null_mut(),
            root_count: 1,
            _reserved_u32: 0,
            roots: [obj],
        };
        scoop_gc_frame_push(&mut frame);
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 1);
        assert_eq!(RELEASE_CALLS.load(Ordering::SeqCst), 0);
        // moving/compaction backend（例如 Immix）可能会更新 roots 槽位到新地址；
        // release callback 应以“对象最终被回收时的地址”为准。
        let obj_after_gc = frame.roots[0];

        // 4) pop roots 后再 collect：对象应被回收，release callback 只被调用一次。
        scoop_gc_frame_pop(&mut frame);
        scoop_gc_collect();
        assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        assert_eq!(RELEASE_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(LAST_RELEASED.load(Ordering::SeqCst), obj_after_gc as usize);

        // 5) 再次 collect：不应二次调用 release callback。
        scoop_gc_collect();
        assert_eq!(RELEASE_CALLS.load(Ordering::SeqCst), 1);

        scoop_thread_unregister();
    }
}
