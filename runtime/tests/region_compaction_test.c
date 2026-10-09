#include "nursery_fixture.h"

#define PAIR_COUNT 1800
#define WIDE_SIZE 30000

static size_t released;

static void release_wide(void *object) {
    assert(((unsigned char *)object)[WIDE_SIZE - 1] == 101);
    released++;
}

static const ScoopTypeDescriptor wide_td = {
    .type_id = 910,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = WIDE_SIZE,
                       .instance_alignment = 16},
    .release_hook = release_wide,
};

static void compact_round(bool pin_region) {
    void *objects[2 * PAIR_COUNT] = {0};
    void **slots[2 * PAIR_COUNT];
    for (size_t index = 0; index < 2 * PAIR_COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 2 * PAIR_COUNT);
    for (size_t index = 0; index < PAIR_COUNT; index++) {
        objects[index] = nursery_allocate(&wide_td, WIDE_SIZE);
        ((unsigned char *)objects[index])[WIDE_SIZE - 1] = 101;
        ((ScoopObjectHeader *)objects[index])->gc_word = GC_RELEASE_READY_BIT;
        objects[PAIR_COUNT + index] = nursery_node(index);
    }
    ScoopGcMetrics before = nursery_metrics();
    assert(before.region_count >= 4);
    size_t released_before = released;
    // One wide survivor makes the destination uniquely denser than the node-only regions.
    uintptr_t destination = scoop_heap_pointer_block(objects[0])->region->base;
    uintptr_t prior[PAIR_COUNT], regions[PAIR_COUNT];
    uintptr_t pinned_region = 0;
    ScoopPinFrame pin;
    for (size_t index = 0; index < PAIR_COUNT; index++) {
        NurseryNode *node = objects[PAIR_COUNT + index];
        prior[index] = (uintptr_t)node;
        regions[index] = scoop_heap_pointer_block(node)->region->base;
        if (index != 0) {
            objects[index] = NULL;
        }
        node->next = objects[PAIR_COUNT + (index + 791) % PAIR_COUNT];
        scoop_rt_gc_write_barrier(&node->next, sizeof node->next);
        if (pin_region && pinned_region == 0 && regions[index] != destination) {
            pinned_region = regions[index];
            scoop_rt_push_pin_frame(&pin, node);
        }
    }
    assert(!pin_region || pinned_region != 0);
    nursery_collect(false);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.region_count <= (pin_region ? 3 : 2));
    assert(after.unmapped_bytes >= before.unmapped_bytes + GC_REGION_SIZE);
    assert(released == released_before + PAIR_COUNT - 1);
    size_t moved = 0;
    for (size_t index = 0; index < PAIR_COUNT; index++) {
        NurseryNode *node = objects[PAIR_COUNT + index];
        assert(node->value == index);
        assert(node->next == objects[PAIR_COUNT + (index + 791) % PAIR_COUNT]);
        uintptr_t current_region = scoop_heap_pointer_block(node)->region->base;
        assert(current_region == destination || current_region == pinned_region);
        bool source = regions[index] != destination && regions[index] != pinned_region;
        assert(((uintptr_t)node != prior[index]) == source);
        moved += source;
    }
    assert(moved != 0 && scoop_rt_gc_debug_last_moved_count() == moved);
    nursery_collect(false);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    assert(released == released_before + PAIR_COUNT - 1);
    if (pin_region) {
        scoop_rt_pop_pin_frame(&pin);
    }
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(released == released_before + PAIR_COUNT);
    assert(scoop_rt_gc_stats() == 0);
    assert(nursery_metrics().region_count == 1);
    assert(nursery_metrics().mapped_bytes == GC_REGION_SIZE);
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &wide_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    compact_round(false);
    compact_round(true);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("sparse regions compact into live holes and shrink across pinned growth cycles");
}
