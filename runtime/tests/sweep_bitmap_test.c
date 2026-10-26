#include <string.h>

#include "nursery_fixture.h"

typedef struct Resource {
    ScoopObjectHeader header;
    uint64_t index;
} Resource;

static unsigned released[9];

static void release_resource(void *object) {
    const Resource *resource = object;
    assert(resource->index < 9);
    assert((resource->header.gc_word & GC_RELEASE_READY_BIT) == 0);
    assert(++released[resource->index] == 1);
}

static const ScoopTypeDescriptor resource_td = {
    .type_id = 918,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(Resource),
                       .instance_alignment = 8},
    .release_hook = release_resource};

static Resource *place(ScoopGcBlockMeta *block, size_t word, uint64_t index) {
    Resource *resource = (void *)((char *)scoop_heap_block_base(block) + word * sizeof(uint64_t));
    *resource = (Resource){{&resource_td, GC_RELEASE_READY_BIT}, index};
    scoop_heap_record_small_object(block, resource, sizeof *resource);
    return resource;
}

static void check_retained_storage(void *const *objects) {
    for (size_t index = 0; index < 9; index++) {
        if (objects[index] == NULL) {
            continue;
        }
        const Resource *resource = objects[index];
        assert((resource->header.gc_word & GC_FORWARDED_BIT) == 0);
        assert((resource->header.gc_word & GC_RELEASE_READY_BIT) != 0);
        ScoopGcBlockMeta *block = pointer_block(objects[index]);
        uintptr_t begin = (uintptr_t)objects[index] - (uintptr_t)block_base(block);
        uintptr_t end = begin + sizeof(Resource);
        for (size_t line = begin / GC_LINE_SIZE; line <= (end - 1) / GC_LINE_SIZE; line++) {
            assert(bit_test(block->line_occupied, line));
        }
        for (const ScoopGcFreeRun *run = free_runs; run != NULL; run = run->next) {
            if (run->block == block) {
                size_t free_begin = (size_t)run->first_line * GC_LINE_SIZE;
                size_t free_end = free_begin + (size_t)run->line_count * GC_LINE_SIZE;
                assert(end <= free_begin || begin >= free_end);
            }
        }
    }
}

int main(void) {
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &resource_td};
    uintptr_t boundary = 0;
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&boundary);
    assert(nursery_node(2)->value == 2);
    nursery_collect(false);
    /* Several bits per word, adjacent words, empty words, and the last possible start. */
    const size_t words[] = {16, 20, 60, 64, 127, 191, 1000, GC_WORDS_PER_BLOCK - 3};
    void *objects[9] = {0};
    void **slots[9];
    for (size_t index = 0; index < 9; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 9);
    scoop_gc_heap_lock();
    ScoopGcBlockMeta *target = scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    assert(target != NULL);
    for (size_t index = 0; index < 8; index++) {
        objects[index] = place(target, words[index], index);
    }
    /* Place one live source in a second region so target preparation also runs. */
    for (size_t index = 1; index < GC_REGION_BLOCKS; index++) {
        assert(scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true) != NULL);
    }
    ScoopGcBlockMeta *source = scoop_heap_activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    assert(source != NULL && source->region != target->region);
    objects[8] = place(source, 16, 8);
    scoop_gc_heap_unlock();
    ScoopPinFrame pin;
    scoop_rt_push_pin_frame(&pin, objects[1]);
    uintptr_t pinned = (uintptr_t)objects[1];
    uintptr_t moving = (uintptr_t)objects[8];
    const bool keep[] = {false, true, false, true, false, true, false, false, true};
    for (size_t index = 0; index < 9; index++) {
        if (!keep[index]) {
            objects[index] = NULL;
        }
    }
    nursery_collect(false);
    assert(scoop_rt_gc_debug_last_moved_count() == 1);
    assert(nursery_metrics().last_full_source_regions == 1);
    assert((uintptr_t)objects[8] != moving && (uintptr_t)objects[1] == pinned);
    check_retained_storage(objects);
    nursery_collect(false);
    assert(scoop_rt_gc_debug_last_moved_count() == 0);
    check_retained_storage(objects);
    for (size_t index = 0; index < 9; index++) {
        assert(released[index] == (unsigned)!keep[index]);
        if (keep[index]) {
            assert(((Resource *)objects[index])->index == index);
        }
    }
    scoop_rt_pop_pin_frame(&pin);
    scoop_rt_pop_native_roots(&roots);
    nursery_collect(false);
    nursery_collect(false);
    for (size_t index = 0; index < 9; index++) {
        assert(released[index] == 1);
    }
    assert(scoop_rt_gc_stats() == 0);
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
    puts("bitmap sweep preserves boundary starts, pins and exactly-once release after compaction");
}
