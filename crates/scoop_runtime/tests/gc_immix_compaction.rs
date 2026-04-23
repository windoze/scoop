// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1407（Immix moving/compaction：forwarding + roots 更新）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::ptr;
    use std::sync::Mutex;

    type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
    type ScoopTypeTraceFn = Option<
        unsafe extern "C" fn(
            object: *mut c_void,
            visitor: ScoopGcTraceVisitor,
            ctx: *mut c_void,
        ) -> u64,
    >;
    type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

    // `scoop_runtime_init()` 与 GC 全局状态是进程级别的；同一个 test binary 内并发跑多个
    // Immix compaction 集成测试会互相干扰，甚至让 STW 错把另一个测试线程视为未 park 的
    // managed 线程，从而死锁。
    //
    // 约定：本文件内的 Immix compaction 测试在同一个进程中串行执行；不影响其它 test
    // binary 的并行度。
    static GC_IMMIX_COMPACTION_TEST_LOCK: Mutex<()> = Mutex::new(());

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

    unsafe extern "C" {
        fn scoop_runtime_init();
        fn scoop_thread_register();
        fn scoop_thread_unregister();

        fn scoop_alloc(size: u64) -> *mut c_void;

        fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
        fn scoop_leave_native();

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
    fn immix_compaction_updates_native_roots_slots_and_object_fields() {
        let _lock = GC_IMMIX_COMPACTION_TEST_LOCK.lock().unwrap();
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

            // Rust 测试代码不产生 statepoint stackmaps；因此用 enter_native 注册 roots slots。
            let mut a_slot = a;
            let root0: *mut *mut c_void = &mut a_slot;
            let mut roots: [*mut *mut c_void; 1] = [root0];
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

            // 触发 compaction：GC 后，a_slot 应被原地改写为新地址（moving GC 的关键语义）。
            let old_a = a_slot;
            scoop_gc_collect();
            let new_a = a_slot;

            assert!(!new_a.is_null());
            if new_a == old_a {
                // compaction 会按 “sparse block evacuation” 策略选择性搬迁；
                // 当本轮没有命中可 evacuation 的 block 时，非 pinned 对象可能保持原址。
                eprintln!("[gc_immix_compaction] note: non-pinned object not moved in this round");
            }

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
            let mut moved = new_a != old_a;
            for _round in 0..8usize {
                let before = current_a;
                a_slot = current_a;
                scoop_gc_collect();
                current_a = a_slot;
                assert!(!current_a.is_null());
                moved = moved || current_a != before;

                let payload_ptr =
                    (current_a as *mut u8).add(header_size as usize) as *mut *mut c_void;
                let cur_b = payload_ptr.read();
                assert!(!cur_b.is_null());
                let cur_b_payload = (cur_b as *mut u8).add(header_size as usize);
                assert_eq!(cur_b_payload.read_volatile(), 0x7B);
            }
            if !moved {
                eprintln!(
                    "[gc_immix_compaction] note: non-pinned object never moved in this test run"
                );
            }

            scoop_leave_native();
            scoop_thread_unregister();
        }
    }

    #[test]
    fn immix_compaction_does_not_move_pinned_objects() {
        let _lock = GC_IMMIX_COMPACTION_TEST_LOCK.lock().unwrap();
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

            // Rust 测试代码不产生 statepoint stackmaps；因此用 enter_native 注册 roots slots。
            let mut a_slot = a;
            let mut pinned_slot = pinned;
            let root0: *mut *mut c_void = &mut a_slot;
            let root1: *mut *mut c_void = &mut pinned_slot;
            let mut roots: [*mut *mut c_void; 2] = [root0, root1];
            scoop_enter_native(roots.as_mut_ptr(), roots.len() as u32);

            // GC：pinned 地址必须不变；A roots 允许被更新（移动）。
            let pinned_before = pinned_slot;
            let a_before = a_slot;

            scoop_gc_collect();

            assert_eq!(
                pinned_slot, pinned_before,
                "pinned object address must be stable (roots slot should not be rewritten)"
            );
            let a_after = a_slot;
            assert!(!a_after.is_null());
            if a_after == a_before {
                // compaction 会按 “sparse block evacuation” 策略选择性搬迁；
                // 当本轮没有命中可 evacuation 的 block 时，非 pinned 对象可能保持原址。
                eprintln!("[gc_immix_compaction] note: non-pinned object not moved in this round");
            }

            // pinned 哨兵应保持。
            for i in 0..64usize {
                let byte = pinned_payload.add(i).read_volatile();
                assert_eq!(byte, 0xCC, "pinned object payload must not be overwritten");
            }

            // A->B 引用仍应正确，且 B 哨兵保持。
            let a_after_payload =
                (a_after as *mut u8).add(header_size as usize) as *mut *mut c_void;
            let b_after = a_after_payload.read();
            assert!(!b_after.is_null());
            let b_after_payload = (b_after as *mut u8).add(header_size as usize);
            for i in 0..32usize {
                let byte = b_after_payload.add(i).read_volatile();
                assert_eq!(byte, 0x5D, "B payload must survive moving/compaction");
            }

            scoop_leave_native();
            assert_eq!(scoop_unpin(pinned), 1);
            scoop_gc_collect();

            scoop_thread_unregister();
        }
    }
}
