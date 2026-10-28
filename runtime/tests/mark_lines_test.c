#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"

static const ScoopTypeDescriptor bytes_td = {
    .type_id = 1304,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 1,
                       .inline_stride = 1,
                       .inline_alignment = 1},
};

static void *allocate_bytes(size_t size) {
    ScoopArray *array = nursery_allocate(&bytes_td, size);
    array->size = size - 24;
    return array;
}

static void run_case(size_t workers, bool minor, bool old_slots) {
    char setting[8];
    snprintf(setting, sizeof setting, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    assert(setenv("SCOOP_GC_OLD_SLOTS", old_slots ? "2" : "0", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&bytes_td, &nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    assert(nursery_node(7)->value == 7);
    nursery_collect(false);
    assert(nursery_metrics().full_collections == 1);
    scoop_gc_heap_state.nursery_capacity = (size_t)32 << 20;

    static const struct {
        size_t offset;
        size_t size;
    } cases[] = {
        {GC_LINE_SIZE, 24},
        {2 * GC_LINE_SIZE - 8, 40},
        {64 * GC_LINE_SIZE - 24, 24},
        {64 * GC_LINE_SIZE - 8, 40},
        {64 * GC_LINE_SIZE, GC_LINE_SIZE},
        {64 * GC_LINE_SIZE - 8, 128 * GC_LINE_SIZE + 16},
        {GC_LINE_SIZE, GC_REGULAR_MAX},
        {GC_BLOCK_SIZE - 24, 24},
    };
    enum { COUNT = sizeof cases / sizeof *cases };
    void *objects[COUNT] = {0};
    void **slots[COUNT];
    for (size_t index = 0; index < COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, COUNT);
    for (size_t index = 0; index < COUNT; index++) {
        scoop_gc_mutator_invalidate(scoop_thread_current_required()->mutator);
        if (cases[index].offset != GC_LINE_SIZE) {
            /* Dead prefix bytes must not make the entire block appear live. */
            (void)allocate_bytes(cases[index].offset - GC_LINE_SIZE);
        }
        objects[index] = allocate_bytes(cases[index].size);
        ScoopGcBlockMeta *block = pointer_block(objects[index]);
        assert((uintptr_t)objects[index] - (uintptr_t)block_base(block) == cases[index].offset);
        for (size_t earlier = 0; earlier < index; earlier++) {
            assert(pointer_block(objects[earlier]) != block);
        }
    }
    assert(scoop_rt_pin(objects[0]) == objects[0]);
    assert(scoop_rt_pin(objects[3]) == objects[3]);

    if (minor) {
        void *pinned[] = {objects[0], objects[3]};
        for (unsigned pass = 0; pass < 2; pass++) {
            nursery_collect(pass == 0);
            assert(objects[0] == pinned[0] && objects[3] == pinned[1]);
            /* Minor retains aligned and mixed-layout pinned blocks, while other
             * roots can share a copy target. Full collection must consume the result. */
            for (size_t index = 0; index < COUNT; index++) {
                ScoopGcBlockMeta *block = pointer_block(objects[index]);
                uint64_t expected[GC_LINE_BITMAP_WORDS] = {0};
                for (size_t other = 0; other < COUNT; other++) {
                    if (pointer_block(objects[other]) != block) {
                        continue;
                    }
                    assert(((ScoopArray *)objects[other])->size == cases[other].size - 24);
                    size_t offset = (uintptr_t)objects[other] - (uintptr_t)block_base(block);
                    for (size_t line = offset / GC_LINE_SIZE;
                         line <= (offset + cases[other].size - 1) / GC_LINE_SIZE; line++) {
                        bit_set(expected, line);
                    }
                }
                if (pass != 0 || (block->slot_size == 0 &&
                                  block->head->generation == SCOOP_GC_OLD)) {
                    assert(memcmp(block->line_live, expected, sizeof expected) == 0);
                }
                for (size_t word = 0; word < GC_BITMAP_WORDS; word++) {
                    assert(block->marks[word] == 0);
                }
            }
        }
        _exit(0);
    }

    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(scoop_gc_stop_world());
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
    size_t blocks = scoop_gc_heap_begin_collection_locked(minor);
    scoop_gc_mark_begin(minor, blocks);
#if !SCOOP_ENABLE_GC_STRESS
    assert(scoop_gc_marker.workers[0].slot_layout == minor);
#endif
    scoop_gc_mark_roots();
    assert(scoop_gc_mark_finish(minor) == COUNT);
    assert(scoop_gc_marker.active_workers == workers);
    /* Inspect marking before copying or sweeping can establish their own lines. */
    for (size_t index = 0; index < COUNT; index++) {
        ScoopGcBlockMeta *block = pointer_block(objects[index]);
        assert(block->live_bytes == cases[index].size);
        if (!scoop_gc_marker.workers[0].slot_layout) {
            assert(block->movable_live_bytes == (index == 0 || index == 3 ? 0 : cases[index].size));
        }
        size_t start = cases[index].offset / sizeof(uint64_t);
        for (size_t word = 0; word < GC_BITMAP_WORDS; word++) {
            uint64_t expected = word == start / 64 ? UINT64_C(1) << (start % 64) : 0;
            assert(block->marks[word] == expected);
        }
        for (size_t line = 0; line < GC_BLOCK_SIZE / GC_LINE_SIZE; line++) {
            bool overlaps = line * GC_LINE_SIZE < cases[index].offset + cases[index].size &&
                            (line + 1) * GC_LINE_SIZE > cases[index].offset;
            assert(bit_test(block->line_live, line) == overlaps);
        }
    }
    _exit(0);
}

int main(void) {
    alarm(30);
    for (size_t workers = 1; workers <= 4; workers *= 4) {
        for (unsigned mode = 0; mode < 3; mode++) {
            pid_t child = fork();
            assert(child >= 0);
            if (child == 0) {
                run_case(workers, mode != 0, mode == 2);
            }
            int status = 0;
            assert(waitpid(child, &status, 0) == child);
            assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
        }
    }
    puts("marking preserves exact live lines across bitmap words and block edges");
}
