#include "nursery_fixture.h"

static void reset_heap(void **root) {
    *root = NULL;
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

static void link_node(NurseryNode *owner, NurseryNode *child) {
    owner->next = child;
    scoop_rt_gc_write_barrier(&owner->next, sizeof owner->next);
}

static bool remembered(NurseryNode *owner) {
    ScoopGcRegion *region = pointer_block(owner)->region;
    size_t card = ((uintptr_t)&owner->next - region->base) >> GC_CARD_SHIFT;
    return region->cards[card] == 1 &&
           atomic_load(&region->card_groups[card >> GC_CARD_GROUP_SHIFT].next) != NULL;
}

static void check_chain(NurseryNode *node, uint64_t count) {
    uint64_t seen = 0, sum = 0;
    while (node != NULL) {
        assert(seen < count);
        seen++;
        sum += node->value;
        node = node->next;
    }
    assert(seen == count && sum == count * (count - 1) / 2);
}

static void make_chain(void **root, uint64_t count) {
    for (uint64_t index = 0; index < count; index++) {
        NurseryNode *node = nursery_node(index);
        link_node(node, *root);
        *root = node;
    }
}

static void retained_card_without_new_writes(void **root) {
    reset_heap(root);
    *root = nursery_node(90);
    nursery_collect(false);
    NurseryNode *child = nursery_node(91);
    link_node(*root, child);
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    NurseryNode *owner = *root;
    assert(owner->next != child && owner->next->value == 91);
    assert(scoop_gc_is_young_object_locked(owner->next));
    assert(remembered(owner));
    assert(nursery_metrics().promoted_bytes == before.promoted_bytes);
    assert(scoop_gc_heap_state.survivor_objects == 1 && scoop_rt_gc_stats() == 2);

    /* The old owner is the only root of the survivor, with no new barrier. */
    nursery_collect(true);
    owner = *root;
    assert(owner->next->value == 91 && !scoop_gc_is_young_object_locked(owner->next));
    assert(!remembered(owner));
    assert(scoop_gc_heap_state.survivor_objects == 0 && scoop_rt_gc_stats() == 2);
    assert(nursery_metrics().promoted_bytes == before.promoted_bytes + sizeof(NurseryNode));
}

static void survivor_dies_before_promotion(void **root) {
    reset_heap(root);
    *root = nursery_node(90);
    nursery_collect(false);
    NurseryNode *child = nursery_node(91);
    link_node(*root, child);
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    link_node(*root, NULL);
    nursery_collect(true);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.full_collections == before.full_collections);
    assert(after.promoted_bytes == before.promoted_bytes);
    assert(scoop_rt_gc_stats() == 1 && scoop_gc_heap_state.nursery_bytes == 0);
    assert(atomic_load(&scoop_gc_heap_state.dirty_card_groups) == NULL);
}

static void dense_survivors_need_no_copy(void **root) {
    reset_heap(root);
    *root = nursery_node(90);
    nursery_collect(false);
    NurseryNode *owner = *root;
    for (uint64_t index = 0; index < 600; index++) {
        NurseryNode *node = nursery_node(index);
        link_node(node, owner->next);
        link_node(owner, node);
    }
    NurseryNode *first = owner->next;
    nursery_collect(true);
    assert(moved_objects == 0 && owner->next == first);
    assert(scoop_gc_is_young_object_locked(first) && remembered(owner));
    assert(scoop_gc_heap_state.survivor_objects == 600 && scoop_rt_gc_stats() == 601);
    check_chain(owner->next, 600);
    nursery_collect(true);
    assert(moved_objects == 0 && owner->next == first);
    assert(!scoop_gc_is_young_object_locked(first) && !remembered(owner));
    assert(scoop_gc_heap_state.survivor_objects == 0 && scoop_rt_gc_stats() == 601);
    check_chain(owner->next, 600);
}

static void promoted_owner_keeps_new_survivor(void **root) {
    reset_heap(root);
    make_chain(root, 600);
    nursery_collect(true);
    NurseryNode *retained = *root;
    assert(pointer_block(retained)->survivor);
    NurseryNode *child = nursery_node(600);
    NurseryNode *tail = *root;
    while (tail->next != NULL) {
        tail = tail->next;
    }
    link_node(tail, child);
    nursery_collect(true);
    assert(*root == retained && !scoop_gc_is_young_object_locked(*root));
    assert(scoop_gc_is_young_object_locked(tail->next) && remembered(tail));
    assert(scoop_rt_gc_stats() == 601 && scoop_gc_heap_state.survivor_objects == 1);
    check_chain(*root, 601);
    nursery_collect(true);
    assert(!scoop_gc_is_young_object_locked(tail->next) && !remembered(tail));
    assert(scoop_rt_gc_stats() == 601);
    check_chain(*root, 601);
}

static void survivor_capacity_and_mixed_ages(void **root) {
    reset_heap(root);
    const uint64_t count = 12000;
    make_chain(root, count);
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(true);
    uint64_t retained = scoop_gc_heap_state.survivor_objects;
    assert(retained != 0 && retained < count);
    assert(scoop_gc_heap_state.nursery_bytes <= scoop_gc_heap_state.nursery_capacity / 4);
    assert(nursery_metrics().promoted_bytes ==
           before.promoted_bytes + (count - retained) * sizeof(NurseryNode));
    assert(scoop_rt_gc_stats() == count);
    check_chain(*root, count);
    nursery_collect(true);
    assert(scoop_gc_heap_state.survivor_objects == 0 && scoop_rt_gc_stats() == count);
    assert(nursery_metrics().promoted_bytes == before.promoted_bytes + count * sizeof(NurseryNode));
    check_chain(*root, count);
    nursery_collect(true);
    assert(scoop_rt_gc_stats() == count);
    check_chain(*root, count);
}

static void pin_and_full_promotion(void **root) {
    reset_heap(root);
    *root = nursery_node(71);
    nursery_collect(true);
    NurseryNode *owner = *root;
    assert(scoop_rt_pin(owner) == owner);
    link_node(owner, nursery_node(73));
    nursery_collect(true);
    assert(*root == owner && owner->value == 71 && owner->next->value == 73);
    assert(!scoop_gc_is_young_object_locked(owner));
    assert(scoop_gc_is_young_object_locked(owner->next) && remembered(owner));
    assert(scoop_rt_unpin(owner) == owner);
    nursery_collect(false);
    owner = *root;
    assert(owner->next->value == 73 && !scoop_gc_is_young_object_locked(owner->next));
    assert(scoop_rt_gc_stats() == 2);
    assert(scoop_gc_heap_state.young_blocks == NULL && scoop_gc_heap_state.nursery_bytes == 0);
    assert(scoop_gc_heap_state.survivor_objects == 0);
    assert(atomic_load(&scoop_gc_heap_state.dirty_card_groups) == NULL);
}

int main(void) {
    assert(setenv("SCOOP_GC_SURVIVOR", "1", 1) == 0);
    assert(setenv("SCOOP_GC_NURSERY_MAX_MIB", "1", 1) == 0);
#ifdef SCOOP_TEST_SURVIVOR_PARALLEL
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
#else
    assert(setenv("SCOOP_GC_WORKERS", "1", 1) == 0);
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
    retained_card_without_new_writes(&root);
    survivor_dies_before_promotion(&root);
    dense_survivors_need_no_copy(&root);
    promoted_owner_keeps_new_survivor(&root);
    survivor_capacity_and_mixed_ages(&root);
    pin_and_full_promotion(&root);
    reset_heap(&root);
    scoop_rt_pop_native_roots(&frame);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("survivor lifetimes, remembered edges, ages, pinning and counts are preserved");
    return 0;
}
