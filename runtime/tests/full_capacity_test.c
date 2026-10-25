#include <string.h>

#include "nursery_fixture.h"

static size_t released;

static void release_wide(void *object) {
    assert(((unsigned char *)object)[GC_REGULAR_MAX - 1] == 73);
    released++;
}

static const ScoopTypeDescriptor wide_td = {
    .type_id = 917,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = GC_REGULAR_MAX,
                       .instance_alignment = 16},
    .release_hook = release_wide};

static void *old_object(void) {
    ScoopGcBlockMeta *block = scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    assert(block != NULL);
    void *object = (char *)scoop_heap_block_base(block) + GC_LINE_SIZE;
    memset(object, 0, GC_REGULAR_MAX);
    *(ScoopObjectHeader *)object = (ScoopObjectHeader){&wide_td, GC_RELEASE_READY_BIT};
    ((unsigned char *)object)[GC_REGULAR_MAX - 1] = 73;
    scoop_heap_record_small_object(block, object, GC_REGULAR_MAX, false);
    return object;
}

/* Observe the real planning boundary: a rejected plan must not sweep targets. */
static void collect_insufficient_capacity(void *dead) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(scoop_gc_stop_world());
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
    size_t blocks = scoop_gc_heap_begin_collection_locked(false);
    scoop_gc_mark_begin(false, blocks);
    scoop_gc_mark_roots();
    uint64_t live = scoop_gc_mark_finish(false);
    ScoopGcSurvivors survivors;
    assert(!scoop_gc_heap_plan_moving_locked(false, &survivors));
    assert(moved_objects == 0 && released == 0);
    assert(scoop_gc_is_object_start_locked(dead));
    scoop_gc_mark_dispose();
    scoop_gc_heap_finish_collection_locked(live, false, &survivors);
    scoop_gc_set_pin_frames_locked(false);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    scoop_gc_resume_world();
    scoop_thread_pop_managed_anchor(&anchor);
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
    for (size_t index = 0; index < GC_REGION_BLOCKS + 2; index++) {
        objects[index] = old_object();
    }
    scoop_gc_heap_unlock();
    void *dead = objects[GC_REGION_BLOCKS - 1];
    objects[GC_REGION_BLOCKS - 1] = NULL;
    uintptr_t first = (uintptr_t)objects[GC_REGION_BLOCKS];
    uintptr_t second = (uintptr_t)objects[GC_REGION_BLOCKS + 1];
    collect_insufficient_capacity(dead);
    assert(released == 1 && scoop_rt_gc_stats() == GC_REGION_BLOCKS + 1);
    assert((uintptr_t)objects[GC_REGION_BLOCKS] == first);
    assert((uintptr_t)objects[GC_REGION_BLOCKS + 1] == second);
    assert(nursery_metrics().last_full_source_regions == 0);
    /* Freeing two more blocks makes this same source eligible to concentrate. */
    objects[GC_REGION_BLOCKS - 2] = NULL;
    objects[GC_REGION_BLOCKS - 3] = NULL;
    nursery_collect(false);
    assert(released == 3 && scoop_rt_gc_debug_last_moved_count() == 2);
    assert(nursery_metrics().last_full_source_regions == 1);
    assert(nursery_metrics().last_full_target_regions == 1);
    assert((uintptr_t)objects[GC_REGION_BLOCKS] != first);
    assert((uintptr_t)objects[GC_REGION_BLOCKS + 1] != second);
    for (size_t index = 0; index < GC_REGION_BLOCKS + 2; index++) {
        if (objects[index] != NULL) {
            assert(((unsigned char *)objects[index])[GC_REGULAR_MAX - 1] == 73);
        }
    }
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    assert(released == GC_REGION_BLOCKS + 2 && scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("full capacity rejection precedes target sweep and permits later compaction");
}
