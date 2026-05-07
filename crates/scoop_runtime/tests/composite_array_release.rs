// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;
use scoop_runtime::gc_backend::{GC_BACKEND, GC_CAPABILITIES};

use core::ffi::c_void;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
type ScoopCompositeTraceFn = Option<
    unsafe extern "C" fn(
        descriptor: *const ScoopCompositeTransportDescriptor,
        value: *mut c_void,
        visitor: ScoopGcTraceVisitor,
        ctx: *mut c_void,
    ) -> u64,
>;
type ScoopCompositeCopyFn = Option<
    unsafe extern "C" fn(
        descriptor: *const ScoopCompositeTransportDescriptor,
        dst: *mut c_void,
        src: *const c_void,
    ),
>;
type ScoopCompositeDropFn = Option<
    unsafe extern "C" fn(descriptor: *const ScoopCompositeTransportDescriptor, value: *mut c_void),
>;

#[repr(C)]
struct ScoopCompositeTransportDescriptor {
    abi_version: u32,
    storage_kind: u32,
    size_bytes: u64,
    align_bytes: u64,
    gc_slot_offsets: *const u64,
    gc_slot_count: u32,
    _reserved_u32: u32,
    trace_fn: ScoopCompositeTraceFn,
    copy_fn: ScoopCompositeCopyFn,
    drop_fn: ScoopCompositeDropFn,
    type_desc: *const c_void,
}

#[repr(C)]
struct CompositeNoRef {
    value: u64,
}

static DROP_CALLS: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn count_drop(
    _descriptor: *const ScoopCompositeTransportDescriptor,
    _value: *mut c_void,
) {
    DROP_CALLS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_array_builder_new() -> *mut c_void;
    fn scoop_array_builder_push_composite(
        builder: *mut c_void,
        descriptor: *const ScoopCompositeTransportDescriptor,
        value: *const c_void,
    );
    fn scoop_array_builder_build_array_composite(
        builder: *mut c_void,
        descriptor: *const ScoopCompositeTransportDescriptor,
    ) -> *mut c_void;

    fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
    fn scoop_leave_native();
    fn scoop_gc_collect();
}

#[test]
#[cfg_attr(
    any(feature = "gc-minimal", feature = "gc-hosted"),
    ignore = "当前 backend 不支持 native_roots；该测试需要 enter_native roots slots"
)]
fn composite_array_release_drops_elements_on_sweep() {
    assert!(
        std::hint::black_box(GC_CAPABILITIES.native_roots),
        "该测试依赖 native_roots；当前 backend={GC_BACKEND:?}, caps={GC_CAPABILITIES:?}"
    );

    let descriptor = ScoopCompositeTransportDescriptor {
        abi_version: 0,
        storage_kind: 0,
        size_bytes: mem::size_of::<CompositeNoRef>() as u64,
        align_bytes: mem::align_of::<CompositeNoRef>() as u64,
        gc_slot_offsets: ptr::null(),
        gc_slot_count: 0,
        _reserved_u32: 0,
        trace_fn: None,
        copy_fn: None,
        drop_fn: Some(count_drop),
        type_desc: ptr::null(),
    };

    unsafe {
        scoop_runtime_init();
        scoop_thread_register();
        DROP_CALLS.store(0, Ordering::SeqCst);

        let builder = scoop_array_builder_new();
        assert!(!builder.is_null());
        let first = CompositeNoRef { value: 1 };
        let second = CompositeNoRef { value: 2 };
        scoop_array_builder_push_composite(
            builder,
            &descriptor,
            (&first as *const CompositeNoRef).cast::<c_void>(),
        );
        scoop_array_builder_push_composite(
            builder,
            &descriptor,
            (&second as *const CompositeNoRef).cast::<c_void>(),
        );

        let array = scoop_array_builder_build_array_composite(builder, &descriptor);
        assert!(!array.is_null());

        // Building transfers out of the builder and drops the builder-side copies;
        // reset so the assertion below observes the array object's release only.
        DROP_CALLS.store(0, Ordering::SeqCst);

        let mut array_slot = array;
        let root0: *mut *mut c_void = &mut array_slot;
        let mut roots = [root0];
        scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);
        scoop_gc_collect();
        assert_eq!(DROP_CALLS.load(Ordering::SeqCst), 0);

        scoop_leave_native();
        scoop_gc_collect();
        assert_eq!(DROP_CALLS.load(Ordering::SeqCst), 2);

        scoop_thread_unregister();
    }
}
