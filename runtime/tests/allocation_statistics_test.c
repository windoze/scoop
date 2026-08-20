#include <sched.h>
#include <stdatomic.h>

#include "nursery_fixture.h"

enum { THREADS = 4, ROUNDS = 8, OBJECTS = 2000, LARGE_BYTES = 40000 };
static _Atomic(unsigned) remaining;

static void unused_release(void *object) { (void)object; }
static const ScoopTypeDescriptor old_td = {
    .type_id = 811,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = unused_release,
};
static const ScoopTypeDescriptor large_td = {
    .type_id = 812,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = LARGE_BYTES,
                       .instance_alignment = 8},
};

static void *allocate_and_detach(void *argument) {
    (void)argument;
    for (unsigned round = 0; round < ROUNDS; round++) {
        assert(scoop_rt_attach_foreign_thread());
        uintptr_t boundary = 0;
        scoop_thread_enter_managed(&boundary);
        for (unsigned index = 0; index < OBJECTS; index++) {
            (void)nursery_node(index);
        }
        (void)nursery_allocate(&old_td, sizeof(NurseryNode));
        (void)nursery_allocate(&large_td, LARGE_BYTES);
        scoop_thread_leave_managed();
        scoop_rt_detach_foreign_thread();
    }
    atomic_fetch_sub_explicit(&remaining, 1, memory_order_release);
    return NULL;
}

static NurseryNode *check_collection_baselines(void) {
    NurseryNode *head = nursery_node(1);
    assert(scoop_rt_pin(head) == head);
    head->next = nursery_node(2);
    scoop_rt_gc_write_barrier(&head->next, sizeof head->next);
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 2);
    (void)nursery_node(3);
    ScoopThreadState *thread = scoop_thread_current_required();
    void *inline_node = scoop_heap_bump(&thread->allocation.cursor, thread->allocation.limit,
                                         sizeof(NurseryNode), 8);
    assert(inline_node != NULL);
    scoop_runtime_finish_tlab_alloc(inline_node, &nursery_node_td, sizeof(NurseryNode));
    assert(scoop_rt_gc_stats() == 4);
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 2);
    (void)nursery_allocate(&old_td, sizeof(NurseryNode));
    assert(scoop_rt_gc_stats() == 3);
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 3);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 2);
    (void)nursery_allocate(&large_td, LARGE_BYTES);
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 3);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 2);
    return head;
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &old_td, &large_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    uintptr_t boundary = 0;
    scoop_thread_enter_managed(&boundary);
    NurseryNode *head = check_collection_baselines();
    ScoopGcMetrics before = nursery_metrics();
    assert(before.allocated_bytes == 5 * sizeof(NurseryNode) + LARGE_BYTES);
    assert(before.nursery_allocated_bytes == 4 * sizeof(NurseryNode));
    scoop_thread_leave_managed();

    pthread_t threads[THREADS];
    atomic_store_explicit(&remaining, THREADS, memory_order_relaxed);
    for (unsigned index = 0; index < THREADS; index++) {
        assert(pthread_create(&threads[index], NULL, allocate_and_detach, NULL) == 0);
    }
    uint64_t previous_bytes = before.allocated_bytes;
    while (atomic_load_explicit(&remaining, memory_order_acquire) != 0) {
        ScoopGcMetrics sample = nursery_metrics();
        assert(sample.allocated_bytes >= previous_bytes);
        assert(scoop_rt_gc_stats() >= 2);
        previous_bytes = sample.allocated_bytes;
        sched_yield();
    }
    for (unsigned index = 0; index < THREADS; index++) {
        assert(pthread_join(threads[index], NULL) == 0);
    }
    ScoopGcMetrics after = nursery_metrics();
    uint64_t young_bytes = (uint64_t)THREADS * ROUNDS * OBJECTS * sizeof(NurseryNode);
    uint64_t old_bytes = (uint64_t)THREADS * ROUNDS * (sizeof(NurseryNode) + LARGE_BYTES);
    assert(after.allocated_bytes == before.allocated_bytes + young_bytes + old_bytes);
    assert(after.nursery_allocated_bytes == before.nursery_allocated_bytes + young_bytes);

    scoop_thread_enter_managed(&boundary);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 2);
    assert(head->next->value == 2);
    assert(scoop_rt_unpin(head) == head);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    assert(nursery_metrics().allocated_bytes == after.allocated_bytes);
    assert(scoop_rt_gc_stats() == 0);
    puts("allocation statistics survive collection, concurrent queries and repeated detach");
    return 0;
}
