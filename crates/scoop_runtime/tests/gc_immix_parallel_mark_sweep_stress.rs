// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1409c3（并行 mark/sweep stress；多线程 + 跨线程引用）。
#[cfg(feature = "gc-immix")]
mod immix {
    use core::ffi::c_void;
    use core::ptr;
    use std::sync::Arc;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
    type ScoopTypeTraceFn = Option<
        unsafe extern "C" fn(
            object: *mut c_void,
            visitor: ScoopGcTraceVisitor,
            ctx: *mut c_void,
        ) -> u64,
    >;
    type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

    // 对齐 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（最小字段集）。
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

        fn scoop_handle_new(obj: *mut c_void) -> u64;
        fn scoop_handle_get(handle: u64) -> *mut c_void;
        fn scoop_handle_drop(handle: u64) -> u32;

        fn scoop_gc_collect();
        fn scoop_gc_debug_heap_object_count() -> u64;

        fn scoop_pin(obj: *mut c_void) -> u32;
        fn scoop_unpin(obj: *mut c_void) -> u32;
    }

    unsafe fn alloc_node(
        desc: *const ScoopTypeDescriptor,
        header_size: u64,
        node_size: u64,
        ptr0: *mut c_void,
        ptr1: *mut c_void,
        sentinel: u64,
    ) -> *mut c_void {
        unsafe {
            assert!(!desc.is_null());
            let node = scoop_alloc(node_size);
            assert!(!node.is_null());

            let hdr = &mut *(node as *mut ScoopGcObjectHeader);
            hdr.type_desc = desc.cast::<c_void>();

            let payload = (node as *mut u8).add(header_size as usize) as *mut *mut c_void;
            payload.add(0).write(ptr0);
            payload.add(1).write(ptr1);

            let sent = payload.add(2).cast::<u64>();
            sent.write(sentinel);

            node
        }
    }

    unsafe fn read_node_sentinel(node: *mut c_void, header_size: u64) -> u64 {
        unsafe {
            let payload = (node as *mut u8).add(header_size as usize) as *mut *mut c_void;
            payload.add(2).cast::<u64>().read_volatile()
        }
    }

    unsafe fn write_anchor_sentinel(anchor: *mut c_void, header_size: u64, byte: u8) {
        unsafe {
            let payload = (anchor as *mut u8).add(header_size as usize);
            // 只写一小段；用于检测 holes 复用/compaction 不会覆盖 live pinned 对象。
            ptr::write_bytes(payload, byte, 64);
        }
    }

    unsafe fn assert_anchor_sentinel(anchor: *mut c_void, header_size: u64, byte: u8) {
        unsafe {
            let payload = (anchor as *mut u8).add(header_size as usize);
            for i in 0..64usize {
                let b = payload.add(i).read_volatile();
                assert_eq!(b, byte, "pinned anchor payload must not be overwritten");
            }
        }
    }

    fn run_scenario(threads: usize, enable_parallel_mark: bool, enable_parallel_sweep: bool) {
        eprintln!(
            "[gc_immix_parallel_mark_sweep_stress] run_scenario: threads={}, parallel_mark={}, parallel_sweep={}",
            threads,
            if enable_parallel_mark { 1 } else { 0 },
            if enable_parallel_sweep { 1 } else { 0 }
        );

        let prev_mark_env = std::env::var("SCOOP_GC_IMMIX_PARALLEL_MARK").ok();
        let prev_sweep_env = std::env::var("SCOOP_GC_IMMIX_PARALLEL_SWEEP").ok();
        match enable_parallel_mark {
            true => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_MARK", "4") },
            false => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") },
        }
        match enable_parallel_sweep {
            true => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP", "4") },
            false => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP") },
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let ptr_bytes = core::mem::size_of::<*mut c_void>() as u64;
        let node_size = header_size + (2 * ptr_bytes) + 8 + 1024;
        let anchor_size = header_size + 4096;

        // 注意：type descriptor 必须放在堆上（leak），避免线程退出后悬挂指针。
        let bitmap = Box::leak(Box::new([0b11u64]));
        let desc = Box::leak(Box::new(ScoopTypeDescriptor {
            abi_version: 0,
            flags: 0,
            size_bytes: node_size,
            align_bytes: core::mem::align_of::<usize>() as u64,
            trace_start_offset_bytes: header_size,
            trace_bitmap_u64_len: 1,
            _reserved_u32: 0,
            trace_bitmap: bitmap.as_ptr(),
            trace_fn: None,
            release_fn: None,
            type_id: 0,
            parent_type_desc: ptr::null(),
            itable: ptr::null(),
            vtable: ptr::null(),
        }));
        let desc_addr: usize = desc as *const ScoopTypeDescriptor as usize;

        let published = Arc::new(
            (0..threads)
                .map(|_| AtomicUsize::new(0))
                .collect::<Vec<AtomicUsize>>(),
        );
        let start = Arc::new(Barrier::new(threads + 1));
        let stop = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for tid in 0..threads {
            let published = published.clone();
            let start = start.clone();
            let stop = stop.clone();
            let desc_addr = desc_addr;

            handles.push(std::thread::spawn(move || unsafe {
                scoop_thread_register();

                // 每线程 pin 住一个较大对象，逼迫 sweep/holes 复用时避免覆盖 live 对象。
                let anchor = scoop_alloc(anchor_size);
                assert!(!anchor.is_null(), "anchor allocation must succeed");
                assert_eq!(scoop_pin(anchor), 1, "pin must succeed");
                write_anchor_sentinel(anchor, header_size, 0xCD);

                // root 节点：ptr0 用于跨线程引用，ptr1 为本线程链表头（用于制造大量边）。
                let root = alloc_node(
                    desc_addr as *const ScoopTypeDescriptor,
                    header_size,
                    node_size,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    tid as u64,
                );

                // Rust mutator 线程不产生 statepoint stackmaps；用 stable handle 把 root 放进全局 roots 表。
                let root_handle = scoop_handle_new(root);
                assert_ne!(root_handle, 0, "handle_new must succeed for root");

                published[tid].store(root_handle as usize, Ordering::Release);
                start.wait();

                // 建立跨线程引用环：root.ptr0 -> next_thread.root
                let next_handle = published[(tid + 1) % threads].load(Ordering::Acquire) as u64;
                assert_ne!(next_handle, 0, "next root handle must be published");
                let next = scoop_handle_get(next_handle);
                assert!(!next.is_null(), "handle_get(next) must succeed");

                let root_now = scoop_handle_get(root_handle);
                assert!(!root_now.is_null(), "handle_get(root) must succeed");
                let payload = (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;
                payload.add(0).write(next);

                let mut i: usize = 0;
                while !stop.load(Ordering::Relaxed) {
                    // root 可能被 moving/compaction 更新地址：每次都从 handle_get 读取。
                    let root_now = scoop_handle_get(root_handle);
                    assert!(!root_now.is_null(), "handle_get(root) must succeed");
                    let root_payload =
                        (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;

                    let local_head = root_payload.add(1).read();
                    if !local_head.is_null() {
                        assert_eq!(read_node_sentinel(local_head, header_size), tid as u64);
                    }

                    // 把跨线程引用写进每个新 node 的 ptr0，增加跨线程边的数量。
                    let next_root = root_payload.add(0).read();

                    // 周期性“断链”，让旧链条变成垃圾，从而逼迫 sweep/holes 复用路径跑起来。
                    let keep_chain = (i % 64) != 0;
                    let node_prev = if keep_chain {
                        local_head
                    } else {
                        ptr::null_mut()
                    };
                    let node = alloc_node(
                        desc_addr as *const ScoopTypeDescriptor,
                        header_size,
                        node_size,
                        next_root,
                        node_prev,
                        tid as u64,
                    );

                    // 写回链表头（注意：不要在 Rust 局部变量里长期保存 head 指针，GC 不会更新它）。
                    let root_now = scoop_handle_get(root_handle);
                    assert!(!root_now.is_null(), "handle_get(root) must succeed");
                    let payload =
                        (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;
                    payload.add(1).write(node);

                    // 额外制造一小批“立即变成垃圾”的对象，增加 sweep 工作量。
                    if (i % 16) == 0 {
                        let _garbage0 = alloc_node(
                            desc_addr as *const ScoopTypeDescriptor,
                            header_size,
                            node_size,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            tid as u64,
                        );
                        let _garbage1 = alloc_node(
                            desc_addr as *const ScoopTypeDescriptor,
                            header_size,
                            node_size,
                            ptr::null_mut(),
                            ptr::null_mut(),
                            tid as u64,
                        );
                        // 不把 _garbage* 写入任何 root：它们应在后续 GC 中被回收。
                        let _ = (_garbage0, _garbage1);
                    }

                    // 快速一致性检查：root 的 sentinel 必须稳定（否则说明标记/更新出现问题）。
                    assert_eq!(read_node_sentinel(root_now, header_size), tid as u64);

                    // 偶尔检查 pinned anchor 的 payload 未被覆盖。
                    if (i % 256) == 0 {
                        assert_anchor_sentinel(anchor, header_size, 0xCD);
                        std::thread::yield_now();
                    }

                    i += 1;
                }

                // roots drop 后，GC 应能回收；anchor 解除 pin 后也应能回收。
                assert_eq!(
                    scoop_handle_drop(root_handle),
                    1,
                    "handle_drop(root) must succeed"
                );
                assert_anchor_sentinel(anchor, header_size, 0xCD);
                assert_eq!(scoop_unpin(anchor), 1, "unpin must succeed");

                scoop_thread_unregister();
            }));
        }

        start.wait();
        eprintln!("[gc_immix_parallel_mark_sweep_stress] all threads started; begin gc rounds");

        // mutator 运行期间触发多轮 GC：并行开关的各种组合都应稳定通过。
        for _round in 0..12usize {
            unsafe { scoop_gc_collect() };
            std::thread::yield_now();
        }

        eprintln!("[gc_immix_parallel_mark_sweep_stress] gc rounds done; stopping mutators");
        stop.store(true, Ordering::Relaxed);
        for h in handles {
            h.join().unwrap();
        }

        eprintln!("[gc_immix_parallel_mark_sweep_stress] mutators joined; final collect");
        unsafe {
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
            scoop_thread_unregister();
        }

        match prev_sweep_env {
            Some(v) => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP", v) },
            None => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_SWEEP") },
        }
        match prev_mark_env {
            Some(v) => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_MARK", v) },
            None => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") },
        }
    }

    #[test]
    fn immix_parallel_mark_sweep_stress_regression() {
        // 组合回归（N=4）：覆盖并行开关的 4 种组合。
        run_scenario(4, false, false);
        run_scenario(4, true, false);
        run_scenario(4, false, true);
        run_scenario(4, true, true);

        // 额外 stress（N=8）：选择最“热”的组合（mark+sweep 同时开启）。
        run_scenario(8, true, true);
    }
}
