#include "support.h"

typedef struct ThreadProbe {
    bool detached_before;
    bool owns_attachment;
    bool nested_is_borrowed;
    bool stack_bounds_contain_local;
    bool registry_has_main_and_worker;
    bool native_roots_are_thread_local;
    bool mode_transitions_are_exact;
    bool detached_after;
} ThreadProbe;

static void *probe_foreign_thread(void *raw_probe) {
    ThreadProbe *probe = raw_probe;
    int stack_local = 0;
    volatile char managed_stack_boundary = 0;
    probe->detached_before = !scoop_rt_thread_debug_is_attached();
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    probe->nested_is_borrowed = !scoop_rt_attach_foreign_thread();
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    uintptr_t stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t local_address = (uintptr_t)&stack_local;
    probe->stack_bounds_contain_local =
        stack_low <= local_address && local_address < stack_high;
    probe->registry_has_main_and_worker = scoop_rt_thread_debug_count() == 2;

    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    probe->native_roots_are_thread_local = scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&frame);
    probe->native_roots_are_thread_local =
        probe->native_roots_are_thread_local && scoop_rt_gc_debug_native_root_count() == 0;
    scoop_rt_thread_debug_leave_managed();
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;

    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    probe->detached_after = !scoop_rt_thread_debug_is_attached();
    return NULL;
}

static void *detach_with_native_root(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    void *root_value = NULL;
    void **root_slots[] = {&root_value};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, root_slots, 1);
    scoop_rt_thread_debug_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *detach_twice(void *unused) {
    (void)unused;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void *caller_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCallerRootFrame outer;
    ScoopCallerRootFrame inner;
    scoop_rt_push_caller_roots(&outer, NULL, 0);
    scoop_rt_push_caller_roots(&inner, NULL, 0);
    scoop_rt_pop_caller_roots(&outer);
    return NULL;
}

static void *compiler_root_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    ScoopCompilerRootFrame outer;
    ScoopCompilerRootFrame inner;
    scoop_rt_push_compiler_roots(&outer, NULL, 0);
    scoop_rt_push_compiler_roots(&inner, NULL, 0);
    scoop_rt_pop_compiler_roots(&outer);
    return NULL;
}

static void leave_wrong_transition(void) {
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition active = {0};
    ScoopThreadTransition wrong = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&active, (uintptr_t)&managed_stack_pointer);
    scoop_rt_leave_native_safe(&wrong);
}

static void *transition_lifo_violation(void *unused) {
    (void)unused;
    volatile char managed_stack_boundary = 0;
    (void)scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    leave_wrong_transition();
    return NULL;
}

static bool thread_protocol_aborts(void *(*start)(void *)) {
    fflush(stdout);
    pid_t pid = fork();
    if (pid == 0) {
        pthread_t thread;
        if (pthread_create(&thread, NULL, start, NULL) != 0) {
            _exit(2);
        }
        (void)pthread_join(thread, NULL);
        _exit(0);
    }
    int status = 0;
    waitpid(pid, &status, 0);
    return WIFSIGNALED(status);
}

typedef struct StwProbe {
    _Atomic bool *collect_now;
    _Atomic bool *stop;
    _Atomic bool ready;
    _Atomic bool collection_returned;
    bool owns_attachment;
    bool root_survived;
    bool mode_transitions_are_exact;
} StwProbe;

typedef struct BorrowedProbe {
    _Atomic bool ready;
    _Atomic bool enter_runtime;
    bool owns_attachment;
    bool root_survived;
    bool mode_transitions_are_exact;
} BorrowedProbe;

enum { MULTI_ALLOC_THREADS = 4, MULTI_ALLOC_BATCH = 512 };

typedef struct MultiAllocProbe {
    _Atomic bool *start;
    _Atomic bool *collection_finished;
    _Atomic uint32_t *ready;
    uint64_t final_handle;
    bool valid_after_collection;
    bool owns_attachment;
} MultiAllocProbe;

static void *multi_allocator(void *raw_probe) {
    MultiAllocProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);

    while (!atomic_load_explicit(probe->start, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }

    ScoopNode *head = NULL;
    void **root_slots[] = {(void **)&head};
    ScoopNativeRootFrame root_frame;
    scoop_rt_push_native_roots(&root_frame, root_slots, 1);
    for (int i = 0; i < MULTI_ALLOC_BATCH; i++) {
        head = new_node(i, head);
    }
    uint64_t first_handle = scoop_rt_get_handle(head);
    atomic_fetch_add_explicit(probe->ready, 1, memory_order_acq_rel);
    while (!atomic_load_explicit(probe->collection_finished, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    const ScoopNode *after_collection = scoop_rt_resolve_handle(first_handle);
    probe->valid_after_collection =
        after_collection != NULL && after_collection->value == MULTI_ALLOC_BATCH - 1;

    for (int i = 0; i < MULTI_ALLOC_BATCH; i++) {
        head = new_node(MULTI_ALLOC_BATCH + i, head);
    }
    probe->final_handle = scoop_rt_get_handle(head);
    (void)scoop_rt_release_handle(first_handle);
    scoop_rt_pop_native_roots(&root_frame);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static void *managed_collection_requester(void *raw_probe) {
    StwProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(probe->collect_now, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    scoop_rt_gc_collect();
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    atomic_store_explicit(&probe->collection_returned, true, memory_order_release);
    while (!atomic_load_explicit(probe->stop, memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static void run_native_safe_observer(StwProbe *probe) {
    ScoopNode **caller_root_value = malloc(sizeof *caller_root_value);
    if (caller_root_value == NULL) {
        abort();
    }
    *caller_root_value = new_node(31415, NULL);
    static const uint64_t caller_root_scan[] = {1, 0};
    ScoopCallerRootEntry caller_root_entries[] = {
        {.base = caller_root_value, .scan = caller_root_scan},
    };
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_safe(&transition, (uintptr_t)&managed_stack_pointer);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(probe->stop, memory_order_acquire)) {
        sched_yield();
    }
    scoop_rt_leave_native_safe(&transition);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    probe->root_survived =
        scoop_rt_gc_debug_is_allocated(*caller_root_value) &&
        (*caller_root_value)->header.td == &node_td &&
        (*caller_root_value)->value == 31415;
    scoop_rt_pop_caller_roots(&caller_frame);
    free(caller_root_value);
}

static void *native_safe_observer(void *raw_probe) {
    StwProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    run_native_safe_observer(probe);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

static void run_native_borrowed_observer(BorrowedProbe *probe) {
    ScoopNode **native_root_value = malloc(sizeof *native_root_value);
    if (native_root_value == NULL) {
        abort();
    }
    *native_root_value = new_node(27182, NULL);
    void **native_root_slots[] = {(void **)native_root_value};
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition transition = {0};
    ScoopNativeRootFrame native_frame;
    volatile char managed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, NULL, 0);
    scoop_rt_enter_native_borrowed(&transition,
                                    (uintptr_t)&managed_stack_pointer);
    probe->mode_transitions_are_exact =
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_BORROWED;
    scoop_rt_push_native_roots(&native_frame, native_root_slots, 1);
    atomic_store_explicit(&probe->ready, true, memory_order_release);
    while (!atomic_load_explicit(&probe->enter_runtime,
                                 memory_order_acquire)) {
        sched_yield();
    }

    /* The collector is already waiting for this native-borrowed thread.
     * The first explicit runtime entry parks it and joins that epoch. */
    scoop_rt_gc_collect();
    probe->root_survived =
        scoop_rt_gc_debug_is_allocated(*native_root_value) &&
        (*native_root_value)->header.td == &node_td &&
        (*native_root_value)->value == 27182;
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_NATIVE_BORROWED;
    scoop_rt_pop_native_roots(&native_frame);
    scoop_rt_leave_native_borrowed(&transition);
    probe->mode_transitions_are_exact =
        probe->mode_transitions_are_exact &&
        scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED;
    scoop_rt_pop_caller_roots(&caller_frame);
    free(native_root_value);
}

static void *native_borrowed_observer(void *raw_probe) {
    BorrowedProbe *probe = raw_probe;
    volatile char managed_stack_boundary = 0;
    probe->owns_attachment = scoop_rt_attach_foreign_thread();
    scoop_rt_thread_debug_enter_managed((uintptr_t)&managed_stack_boundary);
    run_native_borrowed_observer(probe);
    scoop_rt_thread_debug_leave_managed();
    if (probe->owns_attachment) {
        scoop_rt_detach_foreign_thread();
    }
    return NULL;
}

void run_thread_tests(void) {
    /* M13 thread-registration baseline: main and foreign pthreads use the
     * same TLS state/registry, nested attach does not transfer ownership,
     * stack bounds come from pthread APIs, and native roots belong to the
     * current thread state. This first probe only checks registration; later
     * probes exercise concurrent allocation and collection. */
    int main_stack_local = 0;
    uintptr_t main_stack_low = scoop_rt_thread_debug_stack_low();
    uintptr_t main_stack_high = scoop_rt_thread_debug_stack_high();
    uintptr_t main_local_address = (uintptr_t)&main_stack_local;
    scoop_rt_println_boolean(scoop_rt_thread_debug_is_attached() &&
                             scoop_rt_thread_debug_mode() ==
                                 SCOOP_THREAD_DEBUG_MANAGED &&
                             scoop_rt_thread_debug_count() == 1 &&
                             main_stack_low <= main_local_address &&
                             main_local_address < main_stack_high);
    ThreadProbe thread_probe = {0};
    pthread_t probe_thread;
    bool probe_created =
        pthread_create(&probe_thread, NULL, probe_foreign_thread, &thread_probe) == 0;
    bool probe_joined = probe_created && pthread_join(probe_thread, NULL) == 0;
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_before &&
                             thread_probe.owns_attachment && thread_probe.nested_is_borrowed &&
                             thread_probe.stack_bounds_contain_local &&
                             thread_probe.registry_has_main_and_worker &&
                             thread_probe.mode_transitions_are_exact);
    scoop_rt_println_boolean(probe_joined && thread_probe.native_roots_are_thread_local);
    scoop_rt_println_boolean(probe_joined && thread_probe.detached_after &&
                             scoop_rt_thread_debug_count() == 1);
    scoop_rt_println_boolean(thread_protocol_aborts(detach_with_native_root));
    scoop_rt_println_boolean(thread_protocol_aborts(detach_twice));
    scoop_rt_println_boolean(thread_protocol_aborts(caller_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(compiler_root_lifo_violation));
    scoop_rt_println_boolean(thread_protocol_aborts(transition_lifo_violation));

    /* M13 cooperative STW: main and a foreign managed requester race to
     * request the same collection. The winner becomes the sole collector;
     * the loser parks and joins that epoch. A second foreign thread remains
     * native-safe and therefore never enters the wait set. */
    _Atomic bool collect_now = false;
    _Atomic bool stop_stw_workers = false;
    StwProbe managed_probe = {
        .collect_now = &collect_now,
        .stop = &stop_stw_workers,
    };
    StwProbe native_probe = {
        .collect_now = &collect_now,
        .stop = &stop_stw_workers,
    };
    atomic_init(&managed_probe.ready, false);
    atomic_init(&managed_probe.collection_returned, false);
    atomic_init(&native_probe.ready, false);
    atomic_init(&native_probe.collection_returned, false);
    pthread_t managed_thread;
    pthread_t native_thread;
    bool managed_created = pthread_create(
                               &managed_thread, NULL, managed_collection_requester,
                               &managed_probe) == 0;
    bool native_created =
        pthread_create(&native_thread, NULL, native_safe_observer, &native_probe) == 0;
    while (managed_created && native_created &&
           (!atomic_load_explicit(&managed_probe.ready, memory_order_acquire) ||
            !atomic_load_explicit(&native_probe.ready, memory_order_acquire))) {
        sched_yield();
    }
    bool workers_ready = managed_created && native_created &&
                         scoop_rt_thread_debug_count() == 3;
    uint64_t epoch_before = scoop_rt_thread_debug_gc_epoch();
    atomic_store_explicit(&collect_now, true, memory_order_release);
    scoop_rt_gc_collect();
    while (workers_ready &&
           !atomic_load_explicit(&managed_probe.collection_returned,
                                 memory_order_acquire)) {
        scoop_rt_safepoint();
        sched_yield();
    }
    uint64_t epoch_after = scoop_rt_thread_debug_gc_epoch();
    bool first_epoch_coalesced =
        workers_ready && epoch_after == epoch_before + 1 &&
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    for (int i = 0; i < 8; i++) {
        scoop_rt_gc_collect();
    }
    bool consecutive_epochs_parked =
        scoop_rt_thread_debug_gc_epoch() == epoch_after + 8 &&
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    scoop_rt_println_boolean(first_epoch_coalesced && consecutive_epochs_parked);
    atomic_store_explicit(&stop_stw_workers, true, memory_order_release);
    bool managed_joined = managed_created && pthread_join(managed_thread, NULL) == 0;
    bool native_joined = native_created && pthread_join(native_thread, NULL) == 0;
    scoop_rt_println_boolean(managed_joined && native_joined &&
                             managed_probe.owns_attachment &&
                             native_probe.owns_attachment &&
                             native_probe.root_survived &&
                             managed_probe.mode_transitions_are_exact &&
                             native_probe.mode_transitions_are_exact &&
                             scoop_rt_thread_debug_count() == 1);

    /* Per-thread TLABs, heap/root synchronization and generation handles:
     * four managed mutators allocate disjoint ranges concurrently, publish
     * handles, park for one deterministic collection, then refill retired
     * TLABs and allocate a second batch. */
    _Atomic bool multi_start = false;
    _Atomic bool multi_collection_finished = false;
    _Atomic uint32_t multi_ready = 0;
    MultiAllocProbe multi_probes[MULTI_ALLOC_THREADS] = {0};
    pthread_t multi_threads[MULTI_ALLOC_THREADS];
    size_t multi_created_count = 0;
    for (size_t i = 0; i < MULTI_ALLOC_THREADS; i++) {
        multi_probes[i].start = &multi_start;
        multi_probes[i].collection_finished = &multi_collection_finished;
        multi_probes[i].ready = &multi_ready;
        if (pthread_create(&multi_threads[i], NULL, multi_allocator,
                           &multi_probes[i]) != 0) {
            break;
        }
        multi_created_count++;
    }
    atomic_store_explicit(&multi_start, true, memory_order_release);
    while (multi_created_count != 0 &&
           atomic_load_explicit(&multi_ready, memory_order_acquire) <
               multi_created_count) {
        scoop_rt_safepoint();
        sched_yield();
    }
    if (multi_created_count != 0) {
        scoop_rt_gc_collect();
    }
    atomic_store_explicit(&multi_collection_finished, true,
                          memory_order_release);
    bool multi_valid = multi_created_count == MULTI_ALLOC_THREADS;
    for (size_t i = 0; i < multi_created_count; i++) {
        multi_valid = pthread_join(multi_threads[i], NULL) == 0 && multi_valid;
        const ScoopNode *node =
            scoop_rt_resolve_handle(multi_probes[i].final_handle);
        size_t count = 0;
        for (; node != NULL; node = node->next) {
            count++;
        }
        multi_valid = multi_valid && multi_probes[i].owns_attachment &&
                      multi_probes[i].valid_after_collection &&
                      count == 2 * MULTI_ALLOC_BATCH;
        (void)scoop_rt_release_handle(multi_probes[i].final_handle);
    }
    scoop_rt_println_boolean(multi_valid &&
                             scoop_rt_thread_debug_count() == 1);

    /* Native transition ABI: caller roots remain published while the active
     * managed segment is frozen. native-safe returns without participating in
     * GC; native-borrowed may explicitly enter the runtime, collect using its
     * caller/native roots, and resume in borrowed mode before the LIFO leave. */
    void *caller_root_value = NULL;
    void **caller_root_slots[] = {&caller_root_value};
    static const uint64_t caller_root_scan[] = {1, 0};
    ScoopCallerRootEntry caller_root_entries[] = {
        {.base = &caller_root_value, .scan = caller_root_scan},
    };
    ScoopCallerRootFrame caller_frame;
    ScoopThreadTransition safe_transition = {0};
    volatile char safe_stack_pointer = 0;
    scoop_rt_push_caller_roots(&caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_safe(&safe_transition, (uintptr_t)&safe_stack_pointer);
    bool native_safe_active = scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1;
    scoop_rt_leave_native_safe(&safe_transition);
    scoop_rt_pop_caller_roots(&caller_frame);
    scoop_rt_println_boolean(native_safe_active &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0);

    ScoopNode *compiler_root_value = new_node(16180, NULL);
    ScoopCallerRootEntry compiler_root_entries[] = {
        {.base = &compiler_root_value, .scan = caller_root_scan},
    };
    ScoopCompilerRootFrame compiler_frame;
    scoop_rt_push_compiler_roots(&compiler_frame, compiler_root_entries, 1);
    scoop_rt_gc_collect();
    bool compiler_root_survived =
        scoop_rt_thread_debug_compiler_root_count() == 1 &&
        scoop_rt_gc_debug_is_allocated(compiler_root_value) &&
        compiler_root_value->value == 16180;
    scoop_rt_pop_compiler_roots(&compiler_frame);
    scoop_rt_println_boolean(
        compiler_root_survived &&
        scoop_rt_thread_debug_compiler_root_count() == 0);

    ScoopCallerRootFrame borrowed_caller_frame;
    ScoopThreadTransition borrowed_transition = {0};
    volatile char borrowed_stack_pointer = 0;
    scoop_rt_push_caller_roots(&borrowed_caller_frame, caller_root_entries, 1);
    scoop_rt_enter_native_borrowed(&borrowed_transition,
                                    (uintptr_t)&borrowed_stack_pointer);
    ScoopNativeRootFrame borrowed_native_frame;
    scoop_rt_push_native_roots(&borrowed_native_frame, caller_root_slots, 1);
    uint64_t borrowed_epoch = scoop_rt_thread_debug_gc_epoch();
    scoop_rt_gc_collect();
    bool borrowed_collected = scoop_rt_thread_debug_gc_epoch() == borrowed_epoch + 1 &&
                              scoop_rt_thread_debug_transition_depth() == 1 &&
                              scoop_rt_thread_debug_caller_root_count() == 1 &&
                              scoop_rt_gc_debug_native_root_count() == 1;
    scoop_rt_pop_native_roots(&borrowed_native_frame);
    scoop_rt_leave_native_borrowed(&borrowed_transition);
    scoop_rt_pop_caller_roots(&borrowed_caller_frame);
    scoop_rt_println_boolean(borrowed_collected &&
                             scoop_rt_thread_debug_transition_depth() == 0 &&
                             scoop_rt_thread_debug_caller_root_count() == 0 &&
                             scoop_rt_gc_debug_native_root_count() == 0);

    /* A native-borrowed thread is not quiescent merely because it is outside
     * managed code. Another collector waits while it executes ordinary C,
     * then the borrower parks at its first explicit runtime entry. */
    _Atomic bool borrowed_collect_now = false;
    _Atomic bool stop_borrowed_collector = false;
    StwProbe borrowed_collector_probe = {
        .collect_now = &borrowed_collect_now,
        .stop = &stop_borrowed_collector,
    };
    BorrowedProbe borrowed_probe = {0};
    atomic_init(&borrowed_collector_probe.ready, false);
    atomic_init(&borrowed_collector_probe.collection_returned, false);
    atomic_init(&borrowed_probe.ready, false);
    atomic_init(&borrowed_probe.enter_runtime, false);
    pthread_t borrowed_thread;
    pthread_t borrowed_collector_thread;
    bool borrowed_thread_created =
        pthread_create(&borrowed_thread, NULL, native_borrowed_observer,
                       &borrowed_probe) == 0;
    bool borrowed_collector_created =
        pthread_create(&borrowed_collector_thread, NULL,
                       managed_collection_requester,
                       &borrowed_collector_probe) == 0;
    while (borrowed_thread_created && borrowed_collector_created &&
           (!atomic_load_explicit(&borrowed_probe.ready,
                                  memory_order_acquire) ||
            !atomic_load_explicit(&borrowed_collector_probe.ready,
                                  memory_order_acquire))) {
        scoop_rt_safepoint();
        sched_yield();
    }
    ScoopCallerRootFrame borrowed_wait_caller_frame;
    ScoopThreadTransition borrowed_wait_transition = {0};
    volatile char borrowed_wait_stack_pointer = 0;
    scoop_rt_push_caller_roots(&borrowed_wait_caller_frame, NULL, 0);
    scoop_rt_enter_native_safe(&borrowed_wait_transition,
                               (uintptr_t)&borrowed_wait_stack_pointer);
    uint64_t borrowed_wait_epoch = scoop_rt_thread_debug_gc_epoch();
    atomic_store_explicit(&borrowed_collect_now, true,
                          memory_order_release);
    while (borrowed_thread_created && borrowed_collector_created &&
           scoop_rt_thread_debug_gc_epoch() == borrowed_wait_epoch) {
        sched_yield();
    }
    atomic_store_explicit(&borrowed_probe.enter_runtime, true,
                          memory_order_release);
    while (borrowed_thread_created && borrowed_collector_created &&
           !atomic_load_explicit(&borrowed_collector_probe.collection_returned,
                                 memory_order_acquire)) {
        sched_yield();
    }
    bool borrowed_wait_counts =
        scoop_rt_thread_debug_last_gc_parked_count() == 1 &&
        scoop_rt_thread_debug_last_gc_native_safe_count() == 1;
    atomic_store_explicit(&stop_borrowed_collector, true,
                          memory_order_release);
    scoop_rt_leave_native_safe(&borrowed_wait_transition);
    scoop_rt_pop_caller_roots(&borrowed_wait_caller_frame);
    bool borrowed_thread_joined =
        borrowed_thread_created && pthread_join(borrowed_thread, NULL) == 0;
    bool borrowed_collector_joined =
        borrowed_collector_created &&
        pthread_join(borrowed_collector_thread, NULL) == 0;
    scoop_rt_println_boolean(
        borrowed_thread_joined && borrowed_collector_joined &&
        borrowed_wait_counts && borrowed_probe.owns_attachment &&
        borrowed_probe.root_survived &&
        borrowed_probe.mode_transitions_are_exact &&
        borrowed_collector_probe.owns_attachment &&
        borrowed_collector_probe.mode_transitions_are_exact &&
        scoop_rt_thread_debug_count() == 1);
}
