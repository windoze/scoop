#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <unistd.h>

#include "../src/thread/testing.h"
#include "nursery_fixture.h"

typedef enum Request { INCREMENT, MINOR_INCREMENT, MINOR, FULL } Request;

static Request request;
static _Thread_local bool rival;
static _Atomic(bool) waiting;
static bool finished_slice;
static bool finished_minor_slice;

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    if (!rival) {
        return;
    }
    if (point == SCOOP_TEST_COLLECTOR_WAITING) {
        atomic_store_explicit(&waiting, true, memory_order_release);
    } else if (point == SCOOP_TEST_COLLECTOR_RUNNING && request == INCREMENT) {
        finished_slice = scoop_gc_heap_state.incremental_active;
    } else if (point == SCOOP_TEST_COLLECTOR_RUNNING && request == MINOR_INCREMENT) {
        finished_minor_slice = scoop_gc_heap_state.incremental_active &&
                               scoop_gc_heap_state.incremental_minor;
    }
}

static void *collect_first(void *unused) {
    (void)unused;
    rival = true;
    assert(scoop_rt_attach_foreign_thread());
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    switch (request) {
    case INCREMENT:
        scoop_gc_collect_incremental_internal(false);
        break;
    case MINOR_INCREMENT:
        scoop_gc_collect_incremental_internal(true);
        break;
    case MINOR:
        scoop_gc_collect_minor_internal();
        break;
    case FULL:
        assert(scoop_gc_collect_internal());
        break;
    }
    scoop_thread_pop_managed_anchor(&anchor);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

int main(void) {
    alarm(20);
    assert(setenv("SCOOP_GC_INCREMENTAL", "1", 1) == 0);
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    uintptr_t boundary = scoop_rt_thread_debug_stack_high();
    scoop_thread_enter_managed((void *)boundary);
    void *root = NULL;
    void **slots[] = {&root};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    for (uint64_t index = 0; index < 131072; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = root;
        root = node;
    }
    nursery_collect(false);
    for (request = INCREMENT; request <= FULL; request++) {
        if (request == MINOR_INCREMENT) {
            /* The rival must suspend a genuinely young graph before joining full. */
            scoop_gc_heap_state.incremental_enabled = false;
            scoop_gc_heap_state.nursery_capacity = 32 * 1024 * 1024;
            collection_threshold = SIZE_MAX;
            root = NULL;
            for (uint64_t index = 0; index < 131072; index++) {
                NurseryNode *node = nursery_node(index);
                node->next = root;
                root = node;
            }
        }
        atomic_store_explicit(&waiting, false, memory_order_relaxed);
        ScoopGcMetrics before = nursery_metrics();
        pthread_t thread;
        assert(pthread_create(&thread, NULL, collect_first, NULL) == 0);
        while (!atomic_load_explicit(&waiting, memory_order_acquire)) {
            sched_yield();
        }
        /* The rival cannot stop the world until this explicit request joins it. */
        _Alignas(16) uintptr_t frame[4] = {0, 0, boundary, 0};
        scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
        assert(pthread_join(thread, NULL) == 0);
        assert(nursery_metrics().full_collections == before.full_collections + 1);
        assert(!scoop_gc_heap_state.incremental_active && !collection_active);
        NurseryNode *node = root;
        for (uint64_t remaining = 131072; remaining != 0; remaining--) {
            assert(node != NULL && node->value == remaining - 1);
            node = node->next;
        }
        assert(node == NULL);
    }
    assert(finished_slice && finished_minor_slice);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("explicit full requests wait past joined minors and mark slices and coalesce fulls");
}
