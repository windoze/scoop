#include <string.h>

#include "nursery_fixture.h"

extern bool scoop_test_vm_fail_mappings;
static size_t released;

static void release_wide(void *object) {
    assert(((unsigned char *)object)[29999] == 73);
    released++;
}

static const ScoopTypeDescriptor wide_td = {
    .type_id = 913,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = 30000,
                       .instance_alignment = 16},
    .release_hook = release_wide,
};

// Publish actual old objects without automatic nursery collections changing the layout.
static void *old_object(void) {
    ScoopGcBlockMeta *block = scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    assert(block != NULL);
    void *object = (char *)scoop_heap_block_base(block) + GC_LINE_SIZE;
    memset(object, 0, 30000);
    *(ScoopObjectHeader *)object = (ScoopObjectHeader){&wide_td, GC_RELEASE_READY_BIT};
    ((unsigned char *)object)[29999] = 73;
    scoop_heap_record_small_object(block, object, 30000, false);
    return object;
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &wide_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    assert(nursery_node(2)->value == 2);
    nursery_collect(false);
    void *objects[GC_REGION_BLOCKS + 2] = {0};
    void **slots[GC_REGION_BLOCKS + 2];
    for (size_t index = 0; index < GC_REGION_BLOCKS + 2; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, GC_REGION_BLOCKS + 2);
    scoop_gc_heap_lock();
    for (size_t index = 0; index < GC_REGION_BLOCKS; index++) {
        objects[index] = old_object();
    }
    // Two sparse sources, one dense destination with exactly one reusable block.
    objects[GC_REGION_BLOCKS - 1] = NULL;
    objects[GC_REGION_BLOCKS] = old_object();
    for (size_t index = 1; index < GC_REGION_BLOCKS; index++) {
        assert(scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true) != NULL);
    }
    objects[GC_REGION_BLOCKS + 1] = old_object();
    scoop_gc_heap_unlock();
    uintptr_t first = (uintptr_t)objects[GC_REGION_BLOCKS];
    uintptr_t second = (uintptr_t)objects[GC_REGION_BLOCKS + 1];
    scoop_test_vm_fail_mappings = true;
    nursery_collect(false);
    assert(released == 1 && scoop_rt_gc_stats() == GC_REGION_BLOCKS + 1);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    assert((uintptr_t)objects[GC_REGION_BLOCKS] == first);
    assert((uintptr_t)objects[GC_REGION_BLOCKS + 1] == second);
    for (size_t index = 0; index < GC_REGION_BLOCKS + 2; index++) {
        if (objects[index] != NULL) {
            assert(((unsigned char *)objects[index])[29999] == 73);
        }
    }
    scoop_test_vm_fail_mappings = false;
    nursery_collect(false);
    assert(released == 1 && scoop_rt_gc_debug_last_moved_count() == 2);
    assert((uintptr_t)objects[GC_REGION_BLOCKS] != first);
    assert((uintptr_t)objects[GC_REGION_BLOCKS + 1] != second);
    // Moving a single sparse source into the empty cache has no concentration benefit.
    first = (uintptr_t)objects[GC_REGION_BLOCKS];
    second = (uintptr_t)objects[GC_REGION_BLOCKS + 1];
    nursery_collect(false);
    assert(scoop_rt_gc_debug_last_moved_count() == 0 && released == 1);
    assert((uintptr_t)objects[GC_REGION_BLOCKS] == first);
    assert((uintptr_t)objects[GC_REGION_BLOCKS + 1] == second);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(released == GC_REGION_BLOCKS + 2 && scoop_rt_gc_stats() == 0);
    assert(nursery_metrics().region_count == 1);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("full reservation failure preserves roots and skips unprofitable cache migration");
}
