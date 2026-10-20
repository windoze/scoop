#include "nursery_fixture.h"

static void check_dead_block_reuse(void) {
    NurseryNode *first = nursery_node(1);
    NurseryNode *last = NULL;
    ScoopGcBlockMeta *source = pointer_block(first);
    for (size_t i = 0; i < 63; i++) {
        last = nursery_node(i + 2);
    }
    assert(pointer_block(last) == source);
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    assert(source->state == SCOOP_BLOCK_FREE);
    assert(!scoop_rt_gc_debug_is_allocated(first));
    assert(!scoop_rt_gc_debug_is_allocated(last));
    assert(nursery_metrics().promoted_bytes == before.promoted_bytes);
    assert(scoop_rt_gc_stats() == 0);

    NurseryNode *fresh = nursery_node(71);
    assert(pointer_block(fresh) == source);
    assert(fresh->next == NULL && fresh->value == 71);
    assert(!scoop_rt_gc_debug_is_allocated(last));
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 0);
}

static void check_evacuated_block_reuse(void **root) {
    NurseryNode *last = NULL;
    for (uint64_t i = 0; i < 64; i++) {
        NurseryNode *node = nursery_node(i);
        if (i % 8 == 0) {
            node->next = *root;
            *root = node;
        }
        last = node;
    }
    NurseryNode *previous = *root;
    ScoopGcBlockMeta *source = pointer_block(previous);
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    assert(*root != previous && source->state == SCOOP_BLOCK_FREE);
    assert(!scoop_rt_gc_debug_is_allocated(previous));
    assert(!scoop_rt_gc_debug_is_allocated(last));
    assert(nursery_metrics().promoted_bytes - before.promoted_bytes == 8 * sizeof(NurseryNode));
    assert(scoop_rt_gc_stats() == 8);
    NurseryNode *node = *root;
    for (uint64_t i = 64; i != 0; i -= 8) {
        assert(node != NULL && node->value == i - 8);
        assert(!scoop_gc_is_young_object_locked(node));
        node = node->next;
    }
    assert(node == NULL);

    NurseryNode *fresh = nursery_node(91);
    assert(pointer_block(fresh) == source && fresh->next == NULL);
    assert(!scoop_rt_gc_debug_is_allocated(last));
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == 8);
    *root = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *root = NULL;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    for (size_t round = 0; round < 4; round++) {
        check_dead_block_reuse();
        check_evacuated_block_reuse(&root);
    }
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("retired nursery blocks reset metadata and preserve promoted graphs");
    return 0;
}
