// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

// 该测试仅在 `gc-immix` backend 下启用：用于回归 T1409c1（并行标记 v0；可开关）。
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

        fn scoop_enter_native(root_slots: *mut *mut *mut c_void, root_slots_len: u32);
        fn scoop_leave_native();
    }

    struct NativeNoRoots;

    impl NativeNoRoots {
        fn enter() -> Self {
            unsafe {
                scoop_enter_native(ptr::null_mut(), 0);
            }
            Self
        }
    }

    impl Drop for NativeNoRoots {
        fn drop(&mut self) {
            unsafe {
                scoop_leave_native();
            }
        }
    }

    fn wait_at_barrier_in_native(start: &Barrier) {
        // Registered Rust test threads have no stackmaps while blocked in host synchronization.
        let _native = NativeNoRoots::enter();
        start.wait();
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

    fn run_scenario(enable_parallel_mark: bool) {
        eprintln!(
            "[gc_immix_parallel_mark] run_scenario: parallel_mark={}",
            if enable_parallel_mark { 1 } else { 0 }
        );
        let prev_env = std::env::var("SCOOP_GC_IMMIX_PARALLEL_MARK").ok();
        match enable_parallel_mark {
            true => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_MARK", "4") },
            false => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") },
        }

        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);
        }

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>() as u64;
        let ptr_bytes = core::mem::size_of::<*mut c_void>() as u64;
        let node_size = header_size + (2 * ptr_bytes) + 8;

        // 注意：type descriptor 不能放在各线程的栈上，否则线程退出后 type_desc 指针会悬挂，
        // 最终 GC sweep 阶段访问 `type_desc->release_fn` 会触发崩溃。
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

        let threads: usize = 4;
        let published = Arc::new(
            (0..threads)
                .map(|_| AtomicUsize::new(0))
                .collect::<Vec<AtomicUsize>>(),
        );
        let start = Arc::new(Barrier::new(threads + 1));
        // 第二道 barrier：保证所有线程都完成“读取邻居 handle / 补齐跨线程环”后，
        // 主线程才开始 GC 轮次，且任何线程才可能进入热路径并最终 drop handle。
        // 否则某个线程可能在 barrier 后被长时间挂起，等它读邻居 handle 时，
        // 邻居线程早已结束热路径并 drop 掉自己的 root handle（见 line 257），
        // 导致 handle_get 返回 null（正确行为）而误判为失败。
        let linked = Arc::new(Barrier::new(threads + 1));
        let stop = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for tid in 0..threads {
            let published = published.clone();
            let start = start.clone();
            let linked = linked.clone();
            let stop = stop.clone();

            handles.push(std::thread::spawn(move || unsafe {
                scoop_thread_register();

                // root 节点先只写入 sentinel；cross-thread link 在所有线程 publish 后再补齐。
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
                wait_at_barrier_in_native(&start);

                // 建立跨线程引用环：root.ptr0 -> next_thread.root
                let next_handle = published[(tid + 1) % threads].load(Ordering::Acquire) as u64;
                assert_ne!(next_handle, 0, "next root handle must be published");
                let next = scoop_handle_get(next_handle);
                assert!(!next.is_null(), "handle_get(next) must succeed");

                let root_now = scoop_handle_get(root_handle);
                assert!(!root_now.is_null(), "handle_get(root) must succeed");
                let payload = (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;
                payload.add(0).write(next);

                // 跨线程环已补齐：再同步一次，确保没有任何线程仍停在“读邻居 handle”阶段时，
                // 主线程就开始 GC 轮次 / 别的线程进入热路径并 drop handle。
                wait_at_barrier_in_native(&linked);

                // 热路径：持续分配并更新 root.ptr1 作为本线程链表头，逼迫 GC tracing 走大量边。
                let mut i: usize = 0;
                while !stop.load(Ordering::Relaxed) {
                    // 注意：Immix 可能触发 moving/compaction，因此不能把 “链表头” 仅保存在
                    // Rust 局部变量里（GC 不会更新它）。每轮都从 root 的字段里读取最新 head。
                    let root_now = scoop_handle_get(root_handle);
                    assert!(!root_now.is_null(), "handle_get(root) must succeed");
                    let root_payload =
                        (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;
                    let local_head = root_payload.add(1).read();
                    if !local_head.is_null() {
                        assert_eq!(read_node_sentinel(local_head, header_size), tid as u64);
                    }

                    let node = alloc_node(
                        desc_addr as *const ScoopTypeDescriptor,
                        header_size,
                        node_size,
                        ptr::null_mut(),
                        local_head,
                        tid as u64,
                    );

                    // root 可能被 moving/compaction 更新地址：每次都从 handle_get 读取。
                    let root_now = scoop_handle_get(root_handle);
                    assert!(!root_now.is_null(), "handle_get(root) must succeed");
                    let payload =
                        (root_now as *mut u8).add(header_size as usize) as *mut *mut c_void;
                    payload.add(1).write(node);

                    // 轻量一致性检查：root 与链表头的 sentinel 必须稳定（否则说明标记/更新出现问题）。
                    assert_eq!(read_node_sentinel(root_now, header_size), tid as u64);
                    assert_eq!(read_node_sentinel(node, header_size), tid as u64);

                    // 避免单线程跑满 CPU（让主线程更容易插入 collect）。
                    i += 1;
                    if i.is_multiple_of(256) {
                        std::thread::yield_now();
                    }
                }

                assert_eq!(
                    scoop_handle_drop(root_handle),
                    1,
                    "handle_drop(root) must succeed"
                );
                scoop_thread_unregister();
            }));
        }

        wait_at_barrier_in_native(&start);
        // 等待所有线程补齐跨线程环后，再开始 GC 轮次（与 worker 侧的 `linked` 对齐）。
        wait_at_barrier_in_native(&linked);
        eprintln!("[gc_immix_parallel_mark] all threads started; begin gc rounds");

        // 在 mutator 运行期间触发多轮 GC：并行开关打开/关闭两种模式都应稳定通过。
        for _round in 0..12usize {
            unsafe { scoop_gc_collect() };
            std::thread::yield_now();
        }

        eprintln!("[gc_immix_parallel_mark] gc rounds done; stopping mutators");
        stop.store(true, Ordering::Relaxed);
        {
            let _native = NativeNoRoots::enter();
            for h in handles {
                h.join().unwrap();
            }
        }

        eprintln!("[gc_immix_parallel_mark] mutators joined; final collect");
        unsafe {
            // roots 已全部 pop：应能回收到 0。
            scoop_gc_collect();
            assert_eq!(scoop_gc_debug_heap_object_count(), 0);

            scoop_thread_unregister();
        }

        match prev_env {
            Some(v) => unsafe { std::env::set_var("SCOOP_GC_IMMIX_PARALLEL_MARK", v) },
            None => unsafe { std::env::remove_var("SCOOP_GC_IMMIX_PARALLEL_MARK") },
        }
    }

    #[test]
    fn immix_parallel_mark_toggle_smoke() {
        // 先在“关闭”模式下跑一轮，再在“开启”模式下跑一轮：避免依赖外部环境变量。
        eprintln!("[gc_immix_parallel_mark] scenario: parallel_mark=0");
        run_scenario(false);

        eprintln!("[gc_immix_parallel_mark] scenario: parallel_mark=1");
        run_scenario(true);
    }
}
