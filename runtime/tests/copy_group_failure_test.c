#include "nursery_fixture.h"

#undef calloc
extern void *calloc(size_t count, size_t size);

static unsigned groups_until_failure;
static bool group_failed;

void *scoop_test_copy_calloc(size_t count, size_t size) {
    /* A copy group is six pointer-sized words; bitmap and region allocations
     * have different extents. Arm only after all mutator allocation is done. */
    if (groups_until_failure != 0 && count == 1 && size == 6 * sizeof(uintptr_t) &&
        --groups_until_failure == 0) {
        group_failed = true;
        return NULL;
    }
    return calloc(count, size);
}

static void append_nodes(void **root, uint64_t first, uint64_t count) {
    for (uint64_t index = first; index < first + count; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = *root;
        *root = node;
    }
}

static void check_graph(NurseryNode *node, uint64_t count) {
    while (count != 0) {
        assert(node != NULL && node->value == --count);
        node = node->next;
    }
    assert(node == NULL);
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        assert(!block->copy_reserved);
    }
}

int main(void) {
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    nursery_collect(false);
    void *root = NULL;
    void **slots[] = {&root};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);

    const uint64_t count = 16384;
    append_nodes(&root, 0, count);
    void *original = root;
    ScoopGcMetrics before = nursery_metrics();
    uint64_t copied = scoop_gc_heap_state.copied_bytes;
    groups_until_failure = 2;
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(group_failed && groups_until_failure == 0);
    assert(after.promotion_fallbacks == before.promotion_fallbacks + 1);
    assert(after.full_collections == before.full_collections + 1);
    assert(root == original && scoop_gc_heap_state.copied_bytes == copied);
    check_graph(root, count);

    append_nodes(&root, count, count);
    nursery_collect(true);
    assert(scoop_gc_heap_state.copied_bytes > copied);
    assert(scoop_rt_gc_stats() == 2 * count);
    check_graph(root, 2 * count);
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_gc_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("copy grouping failure preserves the source graph and subsequent copying");
}
