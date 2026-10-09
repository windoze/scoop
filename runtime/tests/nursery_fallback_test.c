#include "nursery_fixture.h"

extern bool scoop_test_vm_fail_mappings;

static const ScoopTypeDescriptor wide_td = {
    .type_id = 804,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 30000,
                       .instance_alignment = 16},
};

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &wide_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *roots[3] = {0};
    void **slots[] = {&roots[0], &roots[1], &roots[2]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 3);
    roots[0] = nursery_node(70);
    roots[1] = nursery_allocate(&wide_td, 30000);
    roots[2] = nursery_allocate(&wide_td, 30000);
    ((unsigned char *)roots[1])[29999] = 81;
    ((unsigned char *)roots[2])[29999] = 82;
    ScoopGcMetrics before = nursery_metrics();

    // Fill the current region with real old blocks, leave one target block,
    // then fail the next OS mapping. No production allocation budget is used.
    scoop_gc_heap_lock();
    while (scoop_gc_heap_state.allocation_region->next_block < GC_REGION_BLOCKS - 1) {
        assert(scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR) != NULL);
    }
    scoop_test_vm_fail_mappings = true;
    scoop_gc_heap_unlock();
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.promotion_fallbacks == before.promotion_fallbacks + 1);
    assert(after.full_collections == before.full_collections + 1);
    assert(after.minor_collections == before.minor_collections);
    assert(((NurseryNode *)roots[0])->value == 70);
    assert(((unsigned char *)roots[1])[29999] == 81);
    assert(((unsigned char *)roots[2])[29999] == 82);
    for (size_t index = 0; index < 3; index++) {
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
