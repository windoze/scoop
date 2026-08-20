#include "nursery_fixture.h"

#define HUGE_SIZE (((size_t)1 << 30) + 4096)
#define WIDE_COUNT 1200

static const ScoopTypeDescriptor huge_td = {
    .type_id = 901,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = HUGE_SIZE,
                       .instance_alignment = 16},
};
static const ScoopTypeDescriptor wide_td = {
    .type_id = 902,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 30000,
                       .instance_alignment = 16},
};

static void huge_mapping(void) {
    void *root = nursery_allocate(&huge_td, HUGE_SIZE);
    ScoopPinFrame pin;
    scoop_rt_push_pin_frame(&pin, root);
    unsigned char *bytes = root;
    assert(bytes[HUGE_SIZE - 1] == 0);
    bytes[HUGE_SIZE - 1] = 91;
    bytes[GC_REGION_SIZE + 7] = 43;
    ScoopGcRegion *region = scoop_heap_region_for_address((uintptr_t)root);
    assert(region->large && region->size > ((size_t)1 << 30));
    assert(region->size == scoop_heap_large_mapping_size(HUGE_SIZE));
    for (size_t offset = 0; offset < region->size; offset += GC_CHUNK_SIZE) {
        assert(scoop_heap_region_for_address(region->base + offset) == region);
    }
    // The range straddles several radix leaves and its last object byte.
    scoop_rt_gc_write_barrier(bytes + 65530, HUGE_SIZE - 65530);
    size_t first_card = ((uintptr_t)bytes + 65530 - region->base) >> GC_CARD_SHIFT;
    size_t last_card = ((uintptr_t)bytes + HUGE_SIZE - 1 - region->base) >> GC_CARD_SHIFT;
    assert(region->cards[first_card - 1] == 0 && region->cards[last_card + 1] == 0);
    for (size_t card = first_card; card <= last_card; card++) {
        assert(region->cards[card] == 1);
    }
    nursery_collect(false);
    assert(bytes[HUGE_SIZE - 1] == 91 && bytes[GC_REGION_SIZE + 7] == 43);
    assert(scoop_rt_gc_debug_allocation_size(root) == HUGE_SIZE);
    uintptr_t base = region->base;
    scoop_rt_pop_pin_frame(&pin);
    nursery_collect(false);
    assert(scoop_heap_region_for_address(base) == NULL);
    assert(nursery_metrics().large_mapping_count == 0);
}

static void ordinary_regions(void) {
    void *objects[WIDE_COUNT] = {0};
    void **slots[WIDE_COUNT];
    for (size_t index = 0; index < WIDE_COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, WIDE_COUNT);
    for (size_t index = 0; index < WIDE_COUNT; index++) {
        objects[index] = nursery_allocate(&wide_td, 30000);
        ((unsigned char *)objects[index])[29999] = (unsigned char)index;
    }
    nursery_collect(false);
    assert(nursery_metrics().region_count >= 3);
    ScoopGcRegion *first = scoop_heap_region_for_address((uintptr_t)objects[0]);
    bool different_region = false;
    for (size_t index = 0; index < WIDE_COUNT; index++) {
        assert(((unsigned char *)objects[index])[29999] == (unsigned char)index);
        ScoopGcBlockMeta *block = scoop_heap_pointer_block(objects[index]);
        assert(block != NULL && block->region != NULL);
        different_region |= block->region != first;
    }
    assert(different_region);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &huge_td, &wide_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 3, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    assert(nursery_node(3)->value == 3);
    huge_mapping();
    ordinary_regions();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("region growth and independent mappings exceed the former 1 GiB limit");
}
