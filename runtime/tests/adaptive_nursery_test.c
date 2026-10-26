#include "nursery_fixture.h"

static void verify_chain(NurseryNode *node, uint64_t count) {
    while (count != 0) {
        assert(node != NULL && node->value == --count);
        node = node->next;
    }
    assert(node == NULL);
}

int main(void) {
#ifdef SCOOP_TEST_NURSERY_LIMIT
    assert(setenv("SCOOP_GC_NURSERY_MAX_MIB", "16", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *root = NULL;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    ScoopGcMetrics before = nursery_metrics();
    assert(scoop_gc_heap_state.nursery_capacity == GC_NURSERY_MIN_CAPACITY);

    const uint64_t count = 600000;
    for (uint64_t index = 0; index < count; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = root;
        scoop_rt_gc_write_barrier(&node->next, sizeof node->next);
        root = node;
    }
    verify_chain(root, count);
    ScoopGcMetrics grown = nursery_metrics();
    assert(grown.minor_collections >= before.minor_collections + 4);
    assert(grown.full_collections == before.full_collections);
    size_t capacity = scoop_gc_heap_state.nursery_capacity;
    assert(capacity >= GC_INITIAL_THRESHOLD && capacity <= scoop_gc_heap_state.nursery_limit);
    /* A partially filled large nursery does not spend the old generation's budget. */
    assert(committed_bytes > collection_threshold);
    assert(committed_bytes - scoop_gc_heap_state.nursery_bytes < collection_threshold);
    for (uint64_t index = count; index < count * 2; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = root;
        scoop_rt_gc_write_barrier(&node->next, sizeof node->next);
        root = node;
    }
    capacity = scoop_gc_heap_state.nursery_capacity;
    assert(capacity <= scoop_gc_heap_state.nursery_limit);
#ifdef SCOOP_TEST_NURSERY_LIMIT
    assert(capacity == ((size_t)16 << 20));
#endif
    nursery_collect(false);
    verify_chain(root, count * 2);
    assert(scoop_gc_heap_state.nursery_floor > GC_NURSERY_MIN_CAPACITY);

    root = NULL;
    nursery_collect(false);
    assert(scoop_gc_heap_state.nursery_floor == GC_NURSERY_MIN_CAPACITY);
    assert(scoop_gc_heap_state.nursery_capacity == capacity);
    for (size_t round = 0; round < 2; round++) {
        nursery_node(round);
        nursery_collect(true);
        assert(scoop_gc_heap_state.nursery_capacity == capacity);
    }
    nursery_node(2);
    nursery_collect(true);
    assert(scoop_gc_heap_state.nursery_capacity == capacity / 2);
    for (size_t round = 0; round < 16; round++) {
        nursery_node(round);
        nursery_collect(true);
    }
    assert(scoop_gc_heap_state.nursery_capacity == GC_NURSERY_MIN_CAPACITY);

    /* A tiny retained object and repeated explicit collections cannot inflate it. */
    root = nursery_node(91);
    for (size_t round = 0; round < 8; round++) {
        nursery_collect(true);
        assert(((NurseryNode *)root)->value == 91);
        assert(scoop_gc_heap_state.nursery_capacity == GC_NURSERY_MIN_CAPACITY);
    }
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("adaptive nursery grows with retained graphs and shrinks after their release");
    return 0;
}
