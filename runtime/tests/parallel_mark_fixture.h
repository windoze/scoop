#ifndef SCOOP_TEST_PARALLEL_MARK_FIXTURE_H
#define SCOOP_TEST_PARALLEL_MARK_FIXTURE_H

#include <string.h>

#include "nursery_fixture.h"

#define PARALLEL_NODE_COUNT 16384

typedef struct ParallelNode {
    ScoopObjectHeader header;
    struct ParallelNode *next;
    NurseryNode *shared;
    uint64_t value;
} ParallelNode;

typedef struct ParallelEdge {
    void *left;
    uint64_t padding;
    void *right;
} ParallelEdge;

static const uint64_t parallel_node_scan[] = {2, offsetof(ParallelNode, next),
                                              offsetof(ParallelNode, shared)};
static const uint64_t parallel_element_scan[] = {2, offsetof(ParallelEdge, left),
                                                 offsetof(ParallelEdge, right)};
static const uint64_t parallel_array_scan[] = {SCOOP_REFS_ARRAY, 16, 24, sizeof(ParallelEdge),
                                               (uintptr_t)parallel_element_scan};
static const ScoopTypeDescriptor parallel_node_td = {
    .type_id = 920,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(ParallelNode),
                       .instance_alignment = 8},
    .object_scan = parallel_node_scan,
};
static const ScoopTypeDescriptor parallel_array_td = {
    .type_id = 921,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = sizeof(ParallelEdge),
                       .inline_stride = sizeof(ParallelEdge),
                       .inline_alignment = 8,
                       .inline_scan = parallel_element_scan},
    .object_scan = parallel_array_scan,
};
static const ScoopTypeDescriptor parallel_numbers_td = {
    .type_id = 922,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1,
                       .inline_storage_kind = SCOOP_INLINE_STORAGE_INLINE_V1,
                       .minimum_size = 24,
                       .instance_alignment = 8,
                       .inline_offset = 24,
                       .inline_size = 8,
                       .inline_stride = 8,
                       .inline_alignment = 8},
};
static pthread_t parallel_coordinator;
static size_t parallel_released;

static void parallel_release(void *object) {
    assert(pthread_equal(pthread_self(), parallel_coordinator));
    assert(((NurseryNode *)object)->value == 99);
    parallel_released++;
}

static const ScoopTypeDescriptor parallel_release_td = {
    .type_id = 923,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(NurseryNode),
                       .instance_alignment = 8},
    .release_hook = parallel_release,
};

typedef struct ParallelFixture {
    uintptr_t boundary;
    void *objects[5];
    void **slots[5];
    ScoopNativeRootFrame roots;
} ParallelFixture;

static ParallelEdge *parallel_edges(ParallelFixture *fixture) {
    return (ParallelEdge *)((char *)fixture->objects[0] + 24);
}

static void parallel_start(ParallelFixture *fixture, size_t workers) {
    assert(workers >= 1 && workers <= 8);
    char setting[2] = {(char)('0' + workers), '\0'};
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    const ScoopTypeDescriptor *types[] = {&nursery_node_td, &parallel_node_td, &parallel_array_td,
                                          &parallel_numbers_td, &parallel_release_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 5, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed(&fixture->boundary);
    parallel_coordinator = pthread_self();
    for (size_t index = 0; index < 5; index++) {
        fixture->slots[index] = &fixture->objects[index];
    }
    scoop_rt_push_native_roots(&fixture->roots, fixture->slots, 5);
    ScoopArray *array =
        nursery_allocate(&parallel_array_td, 24 + PARALLEL_NODE_COUNT * sizeof(ParallelEdge));
    array->size = PARALLEL_NODE_COUNT;
    fixture->objects[0] = fixture->objects[4] = array;
    fixture->objects[1] = nursery_node(17);
    fixture->objects[2] = nursery_node(19);
    ((NurseryNode *)fixture->objects[1])->next = fixture->objects[2];
    ((NurseryNode *)fixture->objects[2])->next = fixture->objects[1];
    for (size_t index = 0; index < PARALLEL_NODE_COUNT; index++) {
        ParallelNode *node = nursery_allocate(&parallel_node_td, sizeof *node);
        ParallelEdge *edges = parallel_edges(fixture);
        node->value = index;
        node->next = index == 0 ? NULL : edges[index - 1].left;
        node->shared = fixture->objects[1];
        edges[index].left = node;
        edges[index].right = edges[index % 8].left;
        scoop_rt_gc_write_barrier(&edges[index], sizeof edges[index]);
    }
    array = nursery_allocate(&parallel_numbers_td, 24 + 4096 * sizeof(uint64_t));
    array->size = 4096;
    memset((char *)array + 24, 0xa5, 4096 * sizeof(uint64_t));
    fixture->objects[3] = array;
    for (size_t index = 0; index < 8; index++) {
        NurseryNode *dead = nursery_allocate(&parallel_release_td, sizeof *dead);
        dead->value = 99;
        dead->header.gc_word |= GC_RELEASE_READY_BIT;
    }
}

static void parallel_check_full(size_t workers, uint64_t expected_objects, uint64_t array_tasks) {
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(false);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.last_mark_workers == workers && scoop_rt_gc_stats() == expected_objects);
    uint64_t marked = 0;
    for (size_t index = 0; index < 8; index++) {
        marked += after.worker_marked_objects[index] - before.worker_marked_objects[index];
    }
    assert(marked == expected_objects);
    assert(after.traced_objects - before.traced_objects == expected_objects);
    assert(after.array_tasks - before.array_tasks == array_tasks);
    assert(after.mark_ns > before.mark_ns && after.update_ns > before.update_ns);
}

static void parallel_finish(ParallelFixture *fixture) {
    scoop_rt_pop_native_roots(&fixture->roots);
    nursery_collect(false);
    assert(scoop_rt_gc_stats() == 0 && parallel_released == 8);
    scoop_gc_mark_shutdown();
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
}

#endif
