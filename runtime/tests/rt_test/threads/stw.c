#include "internal.h"

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

void run_thread_stw_tests(void) {
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
}

void run_thread_borrowed_wait_tests(void) {
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
