// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1407（Immix moving/compaction：forwarding + roots 更新）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::ptr;

    type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
    type ScoopTypeTraceFn = Option<
        unsafe extern "C" fn(object: *mut c_void, visitor: ScoopGcTraceVisitor, ctx: *mut c_void) -> u64,
    >;
    type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

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
        type_desc: *const c_void,
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

    // `root_count=2` 的固定版本：用于同时放置 movable roots 与 pinned roots。
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

        fn scoop_alloc(size: u64) -> *mut c_void;

        fn scoop_gc_frame_push(frame: *mut ScoopGcFrame);
        fn scoop_gc_frame_pop(frame: *mut ScoopGcFrame);

        fn scoop_gc_collect();

        fn scoop_pin(obj: *mut c_void) -> u32;
        fn scoop_unpin(obj: *mut c_void) -> u32;
    }

    const IMMIX_BLOCK_SIZE: usize = 32 * 1024;

    fn immix_block_base(ptr: *mut c_void) -> usize {
        let p = ptr as usize;
        p & !(IMMIX_BLOCK_SIZE - 1)
    }

    #[test]
    fn immix_compaction_updates_shadow_stack_roots_and_object_fields() {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

            // 对象 B：只做“哨兵 payload”，不含引用字段（type_desc=NULL）。
            let b_size = header_size + 64;
            let b = scoop_alloc(b_size);
            assert!(!b.is_null());

            // 写入哨兵，验证 moving/compaction 的 memcpy 不会丢数据。
            let b_payload = (b as *mut u8).add(header_size as usize);
            ptr::write_bytes(b_payload, 0x7B, 64);

            // 对象 A：payload 第一个 word 存放一个 GC 指针（指向 B）。
            let a_size = header_size + core::mem::size_of::<*mut c_void>() as u64;
            let a = scoop_alloc(a_size);
            assert!(!a.is_null());

            // type descriptor：从 payload 起始扫描 1 个指针槽位（bit0）。
            let bitmap: [u64; 1] = [0b1];
            let desc = ScoopTypeDescriptor {
                abi_version: 0,
                flags: 0,
                size_bytes: a_size,
                align_bytes: core::mem::align_of::<usize>() as u64,
                trace_start_offset_bytes: header_size,
                trace_bitmap_u64_len: bitmap.len() as u32,
                _reserved_u32: 0,
                trace_bitmap: bitmap.as_ptr(),
                trace_fn: None,
                release_fn: None,
                type_id: 0,
                parent_type_desc: ptr::null(),
                itable: ptr::null(),
                vtable: ptr::null(),
            };

            // 写入 A 的 type_desc，并把 payload[0] 设置为 B。
            let a_hdr = &mut *(a as *mut ScoopGcObjectHeader);
            a_hdr.type_desc = (&desc as *const ScoopTypeDescriptor).cast::<c_void>();

            let a_payload = (a as *mut u8).add(header_size as usize) as *mut *mut c_void;
            a_payload.write(b);

            // 通过 shadow stack roots 保活 A（B 通过 A 的引用字段保活）。
            let mut frame = ScoopGcFrame {
                prev: ptr::null_mut(),
                root_count: 1,
                _reserved_u32: 0,
                roots: [a],
            };
            scoop_gc_frame_push(&mut frame);

            // 触发 compaction：GC 后，frame.roots[0] 应被原地改写为新地址（moving GC 的关键语义）。
            let old_a = frame.roots[0];
            scoop_gc_collect();
            let new_a = frame.roots[0];

            assert!(!new_a.is_null());
            assert_ne!(
                new_a, old_a,
                "compaction should move objects and update shadow stack roots"
            );

            // A.payload[0] 必须指向一个有效的 B，并保持哨兵内容不变。
            let new_a_payload = (new_a as *mut u8).add(header_size as usize) as *mut *mut c_void;
            let new_b = new_a_payload.read();
            assert!(!new_b.is_null());

            let new_b_payload = (new_b as *mut u8).add(header_size as usize);
            for i in 0..64usize {
                let byte = new_b_payload.add(i).read_volatile();
                assert_eq!(byte, 0x7B, "B payload must survive moving/compaction");
            }

            // 多跑几轮（轻量 stress）：每轮都应保持引用正确且不崩溃。
            let mut current_a = new_a;
            for _round in 0..8usize {
                frame.roots[0] = current_a;
                scoop_gc_collect();
                current_a = frame.roots[0];
                assert!(!current_a.is_null());

                let payload_ptr =
                    (current_a as *mut u8).add(header_size as usize) as *mut *mut c_void;
                let cur_b = payload_ptr.read();
                assert!(!cur_b.is_null());
                let cur_b_payload = (cur_b as *mut u8).add(header_size as usize);
                assert_eq!(cur_b_payload.read_volatile(), 0x7B);
            }

            scoop_gc_frame_pop(&mut frame);
            scoop_thread_unregister();
        }
    }

    #[test]
    fn immix_compaction_does_not_move_pinned_objects() {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();

            let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;

            // pinned 对象：地址必须稳定（spec §15.10）。
            let pinned_size = header_size + 128;
            let pinned = scoop_alloc(pinned_size);
            assert!(!pinned.is_null());
            assert_eq!(scoop_pin(pinned), 1);

            let pinned_base = immix_block_base(pinned);

            // 写入哨兵，验证 pinned 对象不会被搬迁或覆盖。
            let pinned_payload = (pinned as *mut u8).add(header_size as usize);
            ptr::write_bytes(pinned_payload, 0xCC, 64);

            // 逼迫后续分配进入新 block：确保“可移动对象”不与 pinned 同 block，
            // 否则 compaction 会按 pin policy 跳过整块 evacuation，导致无法回归“roots 更新”。
            let big_size = header_size + 4096;
            loop {
                let p = scoop_alloc(big_size);
                assert!(!p.is_null());
                if immix_block_base(p) != pinned_base {
                    break;
                }
            }

            // 构造一个可移动的 A -> B 引用图，并通过 roots 保活 A。
            let b_size = header_size + 32;
            let b = scoop_alloc(b_size);
            assert!(!b.is_null());
            let b_payload = (b as *mut u8).add(header_size as usize);
            ptr::write_bytes(b_payload, 0x5D, 32);

            let a_size = header_size + core::mem::size_of::<*mut c_void>() as u64;
            let a = scoop_alloc(a_size);
            assert!(!a.is_null());

            let bitmap: [u64; 1] = [0b1];
            let desc = ScoopTypeDescriptor {
                abi_version: 0,
                flags: 0,
                size_bytes: a_size,
                align_bytes: core::mem::align_of::<usize>() as u64,
                trace_start_offset_bytes: header_size,
                trace_bitmap_u64_len: bitmap.len() as u32,
                _reserved_u32: 0,
                trace_bitmap: bitmap.as_ptr(),
                trace_fn: None,
                release_fn: None,
                type_id: 0,
                parent_type_desc: ptr::null(),
                itable: ptr::null(),
                vtable: ptr::null(),
            };
            let a_hdr = &mut *(a as *mut ScoopGcObjectHeader);
            a_hdr.type_desc = (&desc as *const ScoopTypeDescriptor).cast::<c_void>();
            let a_payload = (a as *mut u8).add(header_size as usize) as *mut *mut c_void;
            a_payload.write(b);

            let mut frame = ScoopGcFrame2 {
                prev: ptr::null_mut(),
                root_count: 2,
                _reserved_u32: 0,
                roots: [a, pinned],
            };
            scoop_gc_frame_push((&mut frame as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());

            // GC：pinned 地址必须不变；A roots 允许被更新（移动）。
            let pinned_before = frame.roots[1];
            let a_before = frame.roots[0];

            scoop_gc_collect();

            assert_eq!(
                frame.roots[1], pinned_before,
                "pinned object address must be stable (roots slot should not be rewritten)"
            );
            assert_ne!(
                frame.roots[0], a_before,
                "non-pinned rooted object should be movable under compaction"
            );

            // pinned 哨兵应保持。
            for i in 0..64usize {
                let byte = pinned_payload.add(i).read_volatile();
                assert_eq!(byte, 0xCC, "pinned object payload must not be overwritten");
            }

            // A->B 引用仍应正确，且 B 哨兵保持。
            let a_after = frame.roots[0];
            let a_after_payload = (a_after as *mut u8).add(header_size as usize) as *mut *mut c_void;
            let b_after = a_after_payload.read();
            assert!(!b_after.is_null());
            let b_after_payload = (b_after as *mut u8).add(header_size as usize);
            for i in 0..32usize {
                let byte = b_after_payload.add(i).read_volatile();
                assert_eq!(byte, 0x5D, "B payload must survive moving/compaction");
            }

            scoop_gc_frame_pop((&mut frame as *mut ScoopGcFrame2).cast::<ScoopGcFrame>());
            assert_eq!(scoop_unpin(pinned), 1);
            scoop_gc_collect();

            scoop_thread_unregister();
        }
    }
}
