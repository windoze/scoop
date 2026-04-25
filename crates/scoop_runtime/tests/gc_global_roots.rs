// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use core::ffi::c_void;
use core::ptr;

type ScoopTypeTraceFn = Option<
    unsafe extern "C" fn(
        object: *mut c_void,
        visitor: unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void),
        ctx: *mut c_void,
    ) -> u64,
>;
type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

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

static mut GLOBAL_ROOT_SLOT: *mut c_void = ptr::null_mut();

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_alloc(size: u64) -> *mut c_void;

    fn scoop_gc_collect();
    fn scoop_gc_debug_heap_object_count() -> u64;
    fn scoop_gc_register_global_root(base: *mut c_void, type_desc: *const ScoopTypeDescriptor);

    fn scoop_handle_new(obj: *mut c_void) -> u64;
    fn scoop_handle_drop(handle: u64) -> u32;
}

#[test]
fn gc_registered_global_root_keeps_object_alive_and_slot_stays_live() {
    unsafe {
        scoop_runtime_init();
        scoop_thread_register();

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let keep = scoop_alloc(header_size + 8);
        assert!(
            !keep.is_null(),
            "expected rooted object allocation to succeed"
        );

        for _ in 0..8 {
            let garbage = scoop_alloc(header_size + 8);
            assert!(!garbage.is_null(), "expected garbage allocation to succeed");
        }

        let root_bitmap: [u64; 1] = [0b1];
        let root_slot_type_desc = ScoopTypeDescriptor {
            abi_version: 0,
            flags: 0,
            size_bytes: core::mem::size_of::<*mut c_void>() as u64,
            align_bytes: core::mem::align_of::<*mut c_void>() as u64,
            trace_start_offset_bytes: 0,
            trace_bitmap_u64_len: root_bitmap.len() as u32,
            _reserved_u32: 0,
            trace_bitmap: root_bitmap.as_ptr(),
            trace_fn: None,
            release_fn: None,
            type_id: 0,
            parent_type_desc: ptr::null(),
            itable: ptr::null(),
            vtable: ptr::null(),
        };

        GLOBAL_ROOT_SLOT = keep;
        scoop_gc_register_global_root(
            (&raw mut GLOBAL_ROOT_SLOT).cast::<c_void>(),
            &root_slot_type_desc,
        );

        if GC_CAPABILITIES.precise_roots_update {
            std::env::set_var("SCOOP_GC_MOVE", "1");
            scoop_gc_collect();
            std::env::remove_var("SCOOP_GC_MOVE");
        } else {
            // 非 moving backend 仅验证“注册的 global root 能保活/回收”这一接口契约。
            scoop_gc_collect();
        }

        assert_eq!(
            scoop_gc_debug_heap_object_count(),
            1,
            "registered global root should keep exactly one live object after collect; backend={GC_BACKEND:?}"
        );

        let rooted_after_gc = GLOBAL_ROOT_SLOT;
        assert!(
            !rooted_after_gc.is_null(),
            "global root slot must still point at a live object after collect; backend={GC_BACKEND:?}"
        );

        let handle = scoop_handle_new(rooted_after_gc);
        let handle_error = if GC_CAPABILITIES.precise_roots_update {
            "global root slot must be updated to the live object address after collect"
        } else {
            "global root slot must still reference a live object after collect"
        };
        assert_ne!(handle, 0, "{handle_error}");
        assert_eq!(scoop_handle_drop(handle), 1);

        GLOBAL_ROOT_SLOT = ptr::null_mut();
        scoop_gc_collect();
        assert_eq!(
            scoop_gc_debug_heap_object_count(),
            0,
            "clearing the registered global root slot should let the object be reclaimed"
        );

        scoop_thread_unregister();
    }
}
