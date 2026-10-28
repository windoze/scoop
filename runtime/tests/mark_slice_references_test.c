#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"

#define ELEMENT_COUNT (1 << 20)

static const uint64_t single_scan[] = {1, 0};
static const uint64_t pair_scan[] = {2, 0, 16};

static uint64_t visited_references(void) {
    return scoop_gc_heap_state.metrics.mark_reference_slots +
           scoop_gc_heap_state.metrics.old_reference_slots;
}

static void run_case(size_t workers, bool paired, bool minor) {
    char setting[8];
    snprintf(setting, sizeof setting, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    size_t stride = paired ? 24 : 8;
    const uint64_t *element_scan = paired ? pair_scan : single_scan;
    const uint64_t array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, stride, (uintptr_t)element_scan};
    const ScoopTypeDescriptor array_td = {
        .type_id = 1300,
        .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                           .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                           .minimum_size = 24,
                           .instance_alignment = 8,
                           .inline_offset = 24,
                           .inline_size = stride,
                           .inline_stride = stride,
                           .inline_alignment = 8,
                           .inline_scan = element_scan},
        .object_scan = array_scan,
    };
    const ScoopTypeDescriptor *types[] = {&array_td, &nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 2, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
    assert(nursery_node(7)->value == 7);
    nursery_collect(false);
    assert(nursery_metrics().full_collections == 1);
    void *root = nursery_allocate(&array_td, 24 + ELEMENT_COUNT * stride);
    ((ScoopArray *)root)->size = ELEMENT_COUNT;
    void **slot = &root;
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, &slot, 1);
    assert(!scoop_gc_is_young_object_locked(root));
    if (minor) {
        scoop_rt_gc_write_barrier((char *)root + 24, ELEMENT_COUNT * stride);
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
    scoop_gc_mark_roots();
    if (minor) {
        assert(scoop_gc_mark_dirty_groups(true, true));
    }
    scoop_gc_mark_publish();
    uint64_t before = visited_references();
    /* Null slots still require scanning. A wide element counts both real references. */
    assert(!scoop_gc_mark_step(false));
    uint64_t visited = visited_references() - before;
    uint64_t limit = (workers < 4 ? workers * 1024 : 4096) * 128;
    if (minor) {
        /* A card group can cross the quota; partial groups have rounded work. */
        uint64_t slack = workers * GC_BLOCK_SIZE / sizeof(void *);
        assert(visited >= limit - slack && visited <= limit + slack);
    } else {
        assert(visited == limit);
    }
    assert(atomic_load(&scoop_gc_marker.pending) != 0);
    assert(scoop_gc_mark_step(true));
    assert(visited_references() - before == ELEMENT_COUNT * (paired ? 2 : 1));
    assert(scoop_gc_mark_complete(minor) == (minor ? 0 : 1));
    _exit(0);
}

int main(void) {
    alarm(30);
    for (size_t workers = 1; workers <= 4; workers *= 4) {
        for (size_t variant = 0; variant < 4; variant++) {
            pid_t child = fork();
            assert(child >= 0);
            if (child == 0) {
                run_case(workers, (variant & 1) != 0, (variant & 2) != 0);
            }
            int status = 0;
            assert(waitpid(child, &status, 0) == child);
            assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
        }
    }
    puts("bounded marking counts actual array and remembered references before draining");
}
