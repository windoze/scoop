#include <string.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"

#define REGION_COUNT (GC_MARK_REGION_CACHE + 1)
#define WIDE_SIZE 30000

static const ScoopTypeDescriptor wide_td = {
    .type_id = 977,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = WIDE_SIZE,
                       .instance_alignment = 8},
    .object_scan = nursery_node_scan,
};

static void scattered_graph(void) {
    void *objects[REGION_COUNT] = {0};
    void **slots[REGION_COUNT];
    for (size_t index = 0; index < REGION_COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, REGION_COUNT);
    scoop_gc_heap_state.nursery_capacity = (size_t)32 << 20;
    for (size_t index = 0; index < REGION_COUNT; index++) {
        scoop_gc_mutator_invalidate(scoop_thread_current_required()->mutator);
        scoop_gc_heap_lock();
        ScoopGcRegion *region = scoop_heap_region_create(GC_REGION_SIZE, false);
        scoop_gc_heap_unlock();
        assert(region != NULL);
        /* The retained empty region's free blocks precede newly mapped storage. */
        for (size_t attempt = 0;; attempt++) {
            assert(attempt < GC_REGION_BLOCKS);
            objects[index] = nursery_allocate(&wide_td, WIDE_SIZE);
            if (pointer_block(objects[index])->region == region) {
                break;
            }
        }
        ((NurseryNode *)objects[index])->value = index;
    }
    assert(nursery_metrics().region_count >= REGION_COUNT);
    for (size_t index = 0; index < REGION_COUNT; index++) {
        NurseryNode *node = objects[index];
        node->next = objects[(index + 13) % REGION_COUNT];
        scoop_rt_gc_write_barrier(&node->next, sizeof node->next);
    }
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(false);
    assert(nursery_metrics().unmapped_bytes > before.unmapped_bytes);
    for (size_t pass = 0; pass < 3; pass++) {
        nursery_collect(pass == 1);
        for (size_t index = 0; index < REGION_COUNT; index++) {
            NurseryNode *node = objects[index];
            assert(node->value == index && node->next == objects[(index + 13) % REGION_COUNT]);
        }
    }
    memset(objects, 0, sizeof objects);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0);
    scoop_rt_pop_native_roots(&roots);
}

int main(void) {
    alarm(30);
    assert(setenv("SCOOP_GC_WORKERS", "4", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &wide_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    assert(nursery_node(3)->value == 3);
    for (size_t round = 0; round < 2; round++) {
        scattered_graph();
    }
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("colliding region views preserve moved graphs across mapping retirement and reuse");
}
