#include "nursery_fixture.h"
#include "../src/gc/mutator_internal.h"

static void release_old(void *object) { (void)object; }

static const ScoopTypeDescriptor old_td = {
    .type_id = 820,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 30000,
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan,
    .release_hook = release_old};
static const ScoopTypeDescriptor wide_td = {
    .type_id = 821,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 30000,
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan};
static const ScoopTypeDescriptor large_td = {
    .type_id = 822,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 70000,
                       .instance_alignment = 8}};

static size_t check_young_index(void) {
    size_t indexed = 0, expected = 0;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        expected += active_head(block) && block->generation == SCOOP_GC_YOUNG;
    }
    for (ScoopGcBlockMeta *block = scoop_gc_heap_state.young_blocks; block != NULL;
         block = block->next_collection) {
        assert(++indexed <= expected);
        assert(active_head(block) && block->generation == SCOOP_GC_YOUNG);
        assert(block->kind == SCOOP_BLOCK_KIND_SMALL);
    }
    assert(indexed == expected && scoop_gc_heap_state.evacuation_targets == NULL);
    return indexed;
}

static void check_available_regions(void) {
    size_t expected = 0, indexed = 0;
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        expected += !region->large && region->next_block < GC_REGION_BLOCKS;
    }
    for (ScoopGcRegion *region = scoop_gc_heap_state.available_regions; region != NULL;
         region = region->next_available) {
        assert(++indexed <= expected);
        assert(!region->large && region->next_block < GC_REGION_BLOCKS);
    }
    assert(indexed == expected);
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &old_td, &wide_td, &large_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 4, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    void *roots[4] = {0};
    void **slots[] = {&roots[0], &roots[1], &roots[2], &roots[3]};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 4);

    /* Real pretenured objects fill more than one ordinary region. */
    for (size_t index = 0; index < 600; index++) {
        NurseryNode *old = nursery_allocate(&old_td, 30000);
        old->next = roots[0];
        old->value = index;
        roots[0] = old;
    }
    roots[3] = nursery_allocate(&large_td, 70000);
    assert(check_young_index() == 0);
    check_available_regions();

    for (size_t round = 0; round < 4; round++) {
        for (size_t index = 0; index < 6; index++) {
            NurseryNode *wide = nursery_allocate(&wide_td, 30000);
            wide->next = roots[1];
            wide->value = round * 10 + index;
            roots[1] = wide;
        }
        roots[2] = nursery_node(77);
        const void *pinned = scoop_rt_pin(roots[2]);
        assert(check_young_index() == 6 + scoop_gc_current_mutator()->reserved_count);
        void *old_head = roots[0], *large = roots[3];
        ScoopGcMetrics before = nursery_metrics();
        nursery_collect(true);
        assert(check_young_index() == 0);
        assert(roots[0] == old_head && roots[3] == large && roots[2] == pinned);
        assert(nursery_metrics().vm_return_ns == before.vm_return_ns);
        NurseryNode *node = roots[1];
        for (size_t index = 6; index != 0; index--) {
            assert(node->value == round * 10 + index - 1);
            assert(!scoop_gc_is_young_object_locked(node));
            node = node->next;
        }
        assert(node == NULL);
        assert(scoop_rt_unpin(pinned) == pinned);
        roots[1] = roots[2] = NULL;
        check_available_regions();
    }
    NurseryNode *node = roots[0];
    for (size_t index = 600; index != 0; index--) {
        assert(node->value == index - 1);
        node = node->next;
    }
    assert(node == NULL);

    /* A full collection also retires a nonempty nursery index and unmaps regions. */
    roots[1] = nursery_node(99);
    assert(check_young_index() == 1 + scoop_gc_current_mutator()->reserved_count);
    roots[0] = roots[3] = NULL;
    nursery_collect(false);
    assert(check_young_index() == 0 && ((NurseryNode *)roots[1])->value == 99);
    check_available_regions();
    scoop_rt_pop_native_roots(&frame);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0 && check_young_index() == 0);
    check_available_regions();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("young and available-region indexes survive promotion, pin, reuse and full reclamation");
    return 0;
}
