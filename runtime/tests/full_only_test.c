#include "nursery_fixture.h"

int main(void) {
    assert(setenv("SCOOP_GC_FULL_ONLY", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *roots[2] = {0};
    void **slots[] = {&roots[0], &roots[1]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 2);
    roots[0] = nursery_node(70);
    nursery_collect(true);
    for (uint64_t index = 0; index < 40000; index++) {
        roots[1] = nursery_node(index);
        NurseryNode *parent = roots[0];
        parent->next = roots[1];
        scoop_rt_gc_write_barrier(&parent->next, sizeof parent->next);
    }
    roots[1] = NULL;
    nursery_collect(true);
    assert(((NurseryNode *)roots[0])->next->value == 39999);
    ScoopGcMetrics metrics = nursery_metrics();
    assert(metrics.minor_collections == 0 && metrics.full_collections >= 3);
    assert(metrics.nursery_allocated_bytes >= 40000 * sizeof(NurseryNode));
    assert(scoop_gc_heap_state.minor_pause_ns == 0);
    assert(scoop_gc_heap_state.full_pause_ns == metrics.pause_ns);
    uint64_t pauses = 0;
    for (size_t index = 0; index < 8; index++) {
        pauses += scoop_gc_heap_state.pause_buckets[index];
    }
    assert(pauses == metrics.full_collections);
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("full-only collection preserves nursery allocation and roots");
    return 0;
}
