#include "nursery_fixture.h"

extern bool scoop_test_vm_fail_mappings;

static const ScoopTypeDescriptor wide_td = {
    .type_id = 804,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 12000,
                       .instance_alignment = 16}};

int main(void) {
    assert(setenv("SCOOP_GC_PROMOTE_IN_PLACE", "1", 1) == 0);
#ifdef SCOOP_TEST_SURVIVOR
    assert(setenv("SCOOP_GC_SURVIVOR", "1", 1) == 0);
    assert(setenv("SCOOP_GC_NURSERY_MAX_MIB", "1", 1) == 0);
    enum { WIDE_COUNT = 24 };
#else
    enum { WIDE_COUNT = 3 };
#endif
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &wide_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    void *roots[WIDE_COUNT + 1] = {0};
    void **slots[WIDE_COUNT + 1];
    for (size_t index = 0; index <= WIDE_COUNT; index++) {
        slots[index] = &roots[index];
    }
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, WIDE_COUNT + 1);
    roots[0] = nursery_node(70);
    for (size_t index = 1; index <= WIDE_COUNT; index++) {
        roots[index] = nursery_allocate(&wide_td, 12000);
        ((unsigned char *)roots[index])[11999] = (unsigned char)(80 + index);
        /* Leave each source sparse so the test really needs evacuation. */
        nursery_allocate(&wide_td, 12000);
    }
    ScoopGcMetrics before = nursery_metrics();

    // Fill the current region with real old blocks, leave one target block,
    // then fail the next OS mapping. No production allocation budget is used.
    scoop_gc_heap_lock();
    while (scoop_gc_heap_state.available_regions->next_block < GC_REGION_BLOCKS - 1) {
        assert(scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true) != NULL);
    }
    scoop_test_vm_fail_mappings = true;
    scoop_gc_heap_unlock();
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.promotion_fallbacks == before.promotion_fallbacks + 1);
    assert(after.full_collections == before.full_collections + 1);
    assert(after.minor_collections == before.minor_collections);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    assert(after.update_ns == before.update_ns);
    assert(((NurseryNode *)roots[0])->value == 70);
    for (size_t index = 1; index <= WIDE_COUNT; index++) {
        assert(((unsigned char *)roots[index])[11999] == 80 + index);
    }
    for (size_t index = 0; index <= WIDE_COUNT; index++) {
        assert(!scoop_gc_is_young_object_locked(roots[index]));
    }

    scoop_test_vm_fail_mappings = false;
    roots[0] = nursery_node(71);
    nursery_collect(true);
    assert(((NurseryNode *)roots[0])->value == 71);
    assert(nursery_metrics().minor_collections == before.minor_collections + 1);
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("partial promotion reservation rolls back before full collection");
    return 0;
}
