#include "nursery_fixture.h"
#include "../src/gc/mutator_internal.h"

static void reset_heap(void **roots) {
    roots[0] = roots[1] = roots[2] = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

static NurseryNode *dense_chain(void **root) {
    NurseryNode *tail = NULL;
    for (uint64_t index = 0; index < 800; index++) {
        NurseryNode *node = nursery_node(index);
        node->next = *root;
        *root = node;
        if (tail == NULL) {
            tail = node;
        }
    }
    for (unsigned index = 0; index < 200; index++) {
        nursery_node(index);
    }
    return tail;
}

static NurseryNode *check_chain(NurseryNode *head) {
    for (uint64_t remaining = 800; remaining != 0; remaining--) {
        assert(head != NULL && head->value == remaining - 1);
        head = head->next;
    }
    return head;
}

static void dense_without_copy(void **roots) {
    reset_heap(roots);
    dense_chain(&roots[0]);
    NurseryNode *before = roots[0];
    roots[1] = before;
    ScoopGcMetrics start = nursery_metrics();
    uint64_t copied = scoop_gc_heap_state.copied_bytes;
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(roots[0] == before && roots[1] == before);
    assert(!scoop_gc_is_young_object_locked(before));
    assert(check_chain(before) == NULL && scoop_rt_gc_stats() == 800);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    assert(scoop_gc_heap_state.copied_bytes == copied && after.update_ns == start.update_ns);
    assert(after.promoted_bytes == start.promoted_bytes + 800 * sizeof(NurseryNode));
    ScoopGcBlockMeta *block = pointer_block(before);
    assert(block->forwarding == NULL && block->generation == SCOOP_GC_OLD);
    bool reusable = false;
    for (ScoopGcFreeRun *run = free_runs; run != NULL; run = run->next) {
        reusable |= run->block == block && run->line_count != 0;
    }
    assert(reusable);
}

static void mixed_sources_and_old_writes(void **roots) {
    reset_heap(roots);
    NurseryNode *tail = dense_chain(&roots[0]);
    NurseryNode *dense = roots[0];
    /* An incomplete TLAB is legal; the next object must occupy another block. */
    scoop_gc_reset_allocation(scoop_gc_current_mutator());
    roots[1] = nursery_node(812);
    roots[2] = roots[1];
    NurseryNode *sparse = roots[1];
    tail->next = sparse;
    scoop_rt_gc_write_barrier(&tail->next, sizeof tail->next);
    assert(pointer_block(dense) != pointer_block(sparse));
    nursery_collect(true);
    assert(roots[0] == dense && roots[1] == roots[2] && roots[1] != sparse);
    assert(check_chain(dense) == roots[1] && ((NurseryNode *)roots[1])->value == 812);
    assert(!scoop_gc_is_young_object_locked(dense));
    assert(!scoop_gc_is_young_object_locked(roots[1]) && scoop_rt_gc_stats() == 801);
    assert(scoop_rt_gc_debug_last_moved_count() == 1);

    /* An in-place promoted block participates in the ordinary old-to-young barrier. */
    roots[1] = roots[2] = NULL;
    tail->next = nursery_node(913);
    scoop_rt_gc_write_barrier(&tail->next, sizeof tail->next);
    nursery_collect(true);
    assert(roots[0] == dense && check_chain(dense)->value == 913);
    assert(!scoop_gc_is_young_object_locked(check_chain(dense)));
    nursery_collect(false);
    assert(check_chain(roots[0])->value == 913 && scoop_rt_gc_stats() == 801);
}

int main(void) {
    assert(setenv("SCOOP_GC_PROMOTE_IN_PLACE", "1", 1) == 0);
#ifdef SCOOP_TEST_PROMOTION_PARALLEL
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
#else
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[3] = {0};
    void **slots[] = {&roots[0], &roots[1], &roots[2]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 3);
    dense_without_copy(roots);
    mixed_sources_and_old_writes(roots);
    reset_heap(roots);
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("dense promotion preserves aliases, sparse relocation, reuse and old writes");
    return 0;
}
