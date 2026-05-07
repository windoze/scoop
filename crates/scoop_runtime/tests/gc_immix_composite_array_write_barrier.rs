// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::mem;
    use core::ptr;

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
        unsafe extern "C" fn(
            descriptor: *const ScoopCompositeTransportDescriptor,
            value: *mut c_void,
        ),
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
    struct ScoopGcObjectHeader {
        next: *mut ScoopGcObjectHeader,
        type_desc: *const c_void,
        size_bytes: u64,
        flags: u32,
        mark: u32,
    }

    #[repr(C)]
    struct CompositeRef {
        slot: *mut c_void,
    }

    #[repr(C)]
    struct ScoopGcImmixBlockHeader {
        magic: u64,
        generation: u8,
    }

    const IMMIX_BLOCK_SIZE: usize = 32 * 1024;
    const IMMIX_BLOCK_MAGIC: u64 = 0x5343_4F4F_5049_4D4D; // "SCOOPIMM"

    const GEN_OLD: u8 = 0;
    const GEN_NURSERY: u8 = 1;

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;
        fn scoop_gc_collect();

        fn scoop_array_builder_new() -> *mut c_void;
        fn scoop_array_builder_push_composite(
            builder: *mut c_void,
            descriptor: *const ScoopCompositeTransportDescriptor,
            value: *const c_void,
        );
        fn scoop_array_builder_build_mutable_array_composite(
            builder: *mut c_void,
            descriptor: *const ScoopCompositeTransportDescriptor,
        ) -> *mut c_void;
        fn scoop_array_set_composite(
            array_obj: *mut c_void,
            index: i64,
            descriptor: *const ScoopCompositeTransportDescriptor,
            value: *const c_void,
        );
    }

    fn immix_block_base(ptr: *const c_void) -> *const ScoopGcImmixBlockHeader {
        let addr = ptr as usize;
        let base = addr & !(IMMIX_BLOCK_SIZE - 1);
        base as *const ScoopGcImmixBlockHeader
    }

    fn immix_generation(ptr: *const c_void) -> u8 {
        let block = immix_block_base(ptr);
        let magic = unsafe { (*block).magic };
        assert_eq!(
            magic, IMMIX_BLOCK_MAGIC,
            "expected immix block allocation (magic=0x{magic:016x})"
        );
        unsafe { (*block).generation }
    }

    #[test]
    fn composite_array_set_promotes_ref_slots_through_write_barrier() {
        unsafe {
            std::env::set_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS", "1");
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
        }

        let header_size = mem::size_of::<ScoopGcObjectHeader>() as u64;
        let young = unsafe { scoop_alloc(header_size + 64) };
        assert!(!young.is_null());
        assert_eq!(immix_generation(young), GEN_NURSERY);

        let filler_size = header_size + 8 * 1024;
        let mut saw_old = false;
        for _ in 0..4096 {
            let p = unsafe { scoop_alloc(filler_size) };
            assert!(!p.is_null());
            if immix_generation(p) == GEN_OLD {
                saw_old = true;
                break;
            }
        }
        assert!(saw_old, "expected nursery to fill and fall back to old");

        let slot_offsets = [0u64];
        let descriptor = ScoopCompositeTransportDescriptor {
            abi_version: 0,
            storage_kind: 0,
            size_bytes: mem::size_of::<CompositeRef>() as u64,
            align_bytes: mem::align_of::<CompositeRef>() as u64,
            gc_slot_offsets: slot_offsets.as_ptr(),
            gc_slot_count: slot_offsets.len() as u32,
            _reserved_u32: 0,
            trace_fn: None,
            copy_fn: None,
            drop_fn: None,
            type_desc: ptr::null(),
        };

        let builder = unsafe { scoop_array_builder_new() };
        assert!(!builder.is_null());
        let empty = CompositeRef {
            slot: ptr::null_mut(),
        };
        unsafe {
            scoop_array_builder_push_composite(
                builder,
                &descriptor,
                (&empty as *const CompositeRef).cast::<c_void>(),
            );
        }
        let array =
            unsafe { scoop_array_builder_build_mutable_array_composite(builder, &descriptor) };
        assert!(!array.is_null());
        assert_eq!(
            immix_generation(array),
            GEN_OLD,
            "test requires an old array receiving a nursery ref"
        );

        let replacement = CompositeRef { slot: young };
        unsafe {
            scoop_array_set_composite(
                array,
                0,
                &descriptor,
                (&replacement as *const CompositeRef).cast::<c_void>(),
            );
        }
        assert_eq!(
            immix_generation(young),
            GEN_OLD,
            "composite array set must apply write barriers to ref slots"
        );

        unsafe {
            scoop_gc_collect();
            scoop_thread_unregister();
            std::env::remove_var("SCOOP_GC_IMMIX_NURSERY_BLOCKS");
        }
    }
}
