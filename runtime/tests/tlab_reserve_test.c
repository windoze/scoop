#include "nursery_fixture.h"
#include "../src/gc/mutator_internal.h"

extern bool scoop_test_vm_fail_mappings;

static void partial_reserve_failure(void **roots) {
    ScoopGcMutator *mutator = scoop_gc_current_mutator();
    roots[0] = nursery_node(10);
    assert(mutator->reserved_count == 3);
    scoop_gc_heap_lock();
    while (scoop_gc_heap_state.available_regions->next_block < GC_REGION_BLOCKS - 1) {
        assert(scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true) != NULL);
    }
    scoop_test_vm_fail_mappings = true;
    scoop_gc_heap_unlock();
    scoop_gc_mutator_invalidate(mutator);
    /* The final existing block succeeds; failure of optional reserves is harmless. */
    roots[1] = nursery_node(11);
    assert(mutator->reserved_count == 0);
    assert(((NurseryNode *)roots[0])->value == 10);
    assert(((NurseryNode *)roots[1])->value == 11);
    assert(scoop_gc_heap_state.nursery_bytes == 5 * GC_BLOCK_SIZE);
    scoop_test_vm_fail_mappings = false;
    nursery_collect(false);
    assert(scoop_gc_mutator_is_retired(mutator) && scoop_rt_gc_stats() == 2);
    roots[0] = roots[1] = NULL;
    nursery_collect(false);
}

static void consume_and_reclaim(void **roots) {
    ScoopGcMutator *mutator = scoop_gc_current_mutator();
    ScoopGcMetrics before = nursery_metrics();
    ScoopGcBlockMeta *blocks[4];
    roots[0] = nursery_node(20);
    blocks[0] = mutator->allocation_block;
    assert(mutator->reserved_count == 3);
    size_t committed = committed_bytes;
    assert(scoop_gc_heap_state.nursery_bytes == 4 * GC_BLOCK_SIZE);
    for (size_t index = 1; index < 4; index++) {
        /* Exhaust only the current range, retaining the owner-only reserves. */
        mutator->allocation.cursor = mutator->allocation.limit;
        roots[index] = nursery_node(20 + index);
        blocks[index] = mutator->allocation_block;
        assert(mutator->reserved_count == 3 - index);
        assert(scoop_gc_heap_state.nursery_bytes == 4 * GC_BLOCK_SIZE);
        assert(committed_bytes == committed);
        for (size_t previous = 0; previous < index; previous++) {
            assert(blocks[index] != blocks[previous]);
        }
    }
    ScoopGcMetrics after = nursery_metrics();
    assert(after.allocated_bytes == before.allocated_bytes + 4 * sizeof(NurseryNode));
    nursery_collect(true);
    assert(scoop_gc_mutator_is_retired(mutator));
    assert(scoop_rt_gc_stats() == 4 && scoop_gc_heap_state.nursery_bytes == 0);
    for (size_t index = 0; index < 4; index++) {
        assert(((NurseryNode *)roots[index])->value == 20 + index);
    }
    /* Unconsumed reserves are retired with the current block, then safely reused. */
    roots[0] = nursery_node(30);
    assert(mutator->reserved_count == 3);
    nursery_collect(true);
    assert(scoop_gc_mutator_is_retired(mutator));
    assert(((NurseryNode *)roots[0])->value == 30);
    for (size_t index = 0; index < 4; index++) {
        roots[index] = NULL;
    }
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

int main(void) {
    assert(setenv("SCOOP_GC_NURSERY_MAX_MIB", "1", 1) == 0);
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[4] = {0};
    void **slots[] = {&roots[0], &roots[1], &roots[2], &roots[3]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 4);
    partial_reserve_failure(roots);
    consume_and_reclaim(roots);
    nursery_node(40);
    assert(scoop_gc_current_mutator()->reserved_count == 3);
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    assert(scoop_gc_mutator_is_retired(scoop_gc_current_mutator()));
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("TLAB reserves preserve capacity, partial failures, moving roots and retirement");
    return 0;
}
