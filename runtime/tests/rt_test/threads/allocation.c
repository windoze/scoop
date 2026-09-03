#include "internal.h"

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


void run_thread_allocation_tests(void) {
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
}
