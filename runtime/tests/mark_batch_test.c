#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "nursery_fixture.h"
#include "../src/gc/mark_internal.h"

#undef stress_move

#define ROOT_COUNT 1025

typedef struct BatchNumber {
    ScoopObjectHeader header;
    uint64_t value;
} BatchNumber;

typedef struct BatchNode {
    ScoopObjectHeader header;
    struct BatchNode *next;
    BatchNumber *number;
    uint64_t value;
} BatchNode;

typedef struct ReorderedNode {
    ScoopObjectHeader header;
    uint64_t value;
    BatchNumber *number;
    void *next;
} ReorderedNode;

static const uint64_t node_scan[] = {2, offsetof(BatchNode, next), offsetof(BatchNode, number)};
static const uint64_t reordered_scan[] = {
    2, offsetof(ReorderedNode, number), offsetof(ReorderedNode, next)};
static const ScoopTypeDescriptor number_td = {
    .type_id = 1302,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(BatchNumber),
                       .instance_alignment = 8},
};
static const ScoopTypeDescriptor node_td = {
    .type_id = 1303,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(BatchNode),
                       .instance_alignment = 8},
    .object_scan = node_scan,
};
static const ScoopTypeDescriptor reordered_td = {
    .type_id = 1304,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(ReorderedNode),
                       .instance_alignment = 8},
    .object_scan = reordered_scan,
};

static void collect_and_check(size_t workers, size_t objects, size_t references) {
    ScoopGcMetrics before = nursery_metrics();
    nursery_collect(false);
    ScoopGcMetrics after = nursery_metrics();
    assert(after.last_mark_workers == workers);
    assert(after.traced_objects - before.traced_objects == objects);
    assert(after.mark_reference_slots - before.mark_reference_slots == references);
    assert(scoop_rt_gc_stats() == objects);
    size_t retained = 0;
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        const ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        assert(worker->live == NULL && worker->pending_count == 0);
        assert(worker->continuation == NULL);
        if (index >= workers || objects == 0) {
            assert(worker->spare_live == NULL);
        }
        for (const ScoopGcLiveChunk *chunk = worker->spare_live; chunk != NULL;
             chunk = chunk->next) {
            retained++;
        }
    }
    assert(retained >= (objects + GC_MARK_CHUNK - 1) / GC_MARK_CHUNK);
    assert(retained <= (objects + GC_MARK_CHUNK - 1) / GC_MARK_CHUNK + workers - 1);
}

static bool is_node(size_t index, unsigned pattern) {
    return pattern == 0 || pattern == 3 ||
           (pattern == 2 && (index % 64 == 0 || index % 64 == 63 || index % 3 == 0));
}

static bool is_reordered(size_t index, unsigned pattern) {
    return pattern == 3 && (index % 64 == 0 || index % 64 == 63 || index % 3 == 0);
}

static void check_bounded_continuation(size_t workers) {
    _Alignas(16) uintptr_t frame[4] = {0};
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
    ScoopManagedAnchor anchor;
    scoop_thread_push_managed_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    assert(scoop_gc_stop_world());
    scoop_gc_heap_lock();
    scoop_gc_roots_lock();
    scoop_gc_set_pin_frames_locked(true);
    scoop_gc_retire_allocations();
    uint64_t references_before = scoop_gc_heap_state.metrics.mark_reference_slots;
    size_t blocks = scoop_gc_heap_begin_collection_locked(false);
    scoop_gc_mark_begin(false, blocks);
    scoop_gc_mark_roots();
    scoop_gc_mark_publish();
    scoop_gc_mark_pool_run(1);
    size_t queued = 0;
    for (size_t index = 0; index < workers; index++) {
        const ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        assert(worker->continuation == NULL && worker->pending_scans == 0);
        queued += worker->queue.count;
    }
    /* A slice has no active parent: every retained successor must be stealable. */
    assert(queued != 0 && atomic_load(&scoop_gc_marker.pending) == queued);
    assert(scoop_gc_mark_step(true));
    assert(atomic_load(&scoop_gc_marker.pending) == 0);
    assert(scoop_gc_mark_complete(false) == ROOT_COUNT * 2);
    assert(scoop_gc_heap_state.metrics.mark_reference_slots - references_before == ROOT_COUNT * 2);
    _exit(0);
}

static void run_case(size_t workers, bool bounded) {
    char setting[8];
    snprintf(setting, sizeof setting, "%zu", workers);
    assert(setenv("SCOOP_GC_WORKERS", setting, 1) == 0);
    const ScoopTypeDescriptor *types[] = {&number_td, &node_td, &reordered_td, &nursery_node_td};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 4, NULL, 0, NULL, 0);
    scoop_thread_attach_main();
    scoop_thread_enter_managed((void *)scoop_rt_thread_debug_stack_high());
#if SCOOP_ENABLE_GC_STRESS
    scoop_gc_heap_state.stress_move = true;
#endif
    void *objects[ROOT_COUNT] = {0};
    void **slots[ROOT_COUNT];
    for (size_t index = 0; index < ROOT_COUNT; index++) {
        slots[index] = &objects[index];
    }
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, ROOT_COUNT);
    /* The first empty batch must also reclaim an unrooted object. */
    assert(nursery_node(7)->value == 7);

    const size_t counts[] = {0, 1, 63, 64, 65, 1023, 1024, 1025};
    for (unsigned pattern = 0; !bounded && pattern < 4; pattern++) {
        for (size_t variant = 0; variant < sizeof counts / sizeof *counts; variant++) {
            size_t count = counts[variant];
            memset(objects, 0, sizeof objects);
            size_t references = 0;
            for (size_t index = 0; index < count; index++) {
                if (is_reordered(index, pattern)) {
                    ReorderedNode *node = nursery_allocate(&reordered_td, sizeof *node);
                    node->value = index;
                    objects[index] = node;
                    references += 2;
                } else if (is_node(index, pattern)) {
                    BatchNode *node = nursery_allocate(&node_td, sizeof *node);
                    node->value = index;
                    objects[index] = node;
                    references += 2;
                } else {
                    BatchNumber *number = nursery_allocate(&number_td, sizeof *number);
                    number->value = index;
                    objects[index] = number;
                }
            }
            collect_and_check(workers, count, references);
            for (size_t index = 0; index < count; index++) {
                if (is_reordered(index, pattern)) {
                    ReorderedNode *node = objects[index];
                    assert(node->value == index && node->next == NULL && node->number == NULL);
                } else if (is_node(index, pattern)) {
                    BatchNode *node = objects[index];
                    assert(node->value == index && node->next == NULL && node->number == NULL);
                } else {
                    assert(((BatchNumber *)objects[index])->value == index);
                }
            }
        }
    }

    /* A continuation discovers both scanned and pointer-free entries, and can
     * fill its pending batch or live chunk while scanning the previous entry. */
    memset(objects, 0, sizeof objects);
    for (size_t index = 0; index < ROOT_COUNT; index++) {
        BatchNumber *number = nursery_allocate(&number_td, sizeof *number);
        number->value = index;
        objects[1] = number;
        if (index % 2 == 0) {
            BatchNode *node = nursery_allocate(&node_td, sizeof *node);
            node->next = objects[0];
            node->number = objects[1];
            node->value = index;
            objects[0] = node;
        } else {
            ReorderedNode *node = nursery_allocate(&reordered_td, sizeof *node);
            node->next = objects[0];
            node->number = objects[1];
            node->value = index;
            objects[0] = node;
        }
        objects[1] = NULL;
    }
    objects[2] = objects[0];
    if (bounded) {
        check_bounded_continuation(workers);
    }
    for (size_t round = 0; round < 2; round++) {
        collect_and_check(workers, ROOT_COUNT * 2, ROOT_COUNT * 2);
        assert(objects[0] == objects[2]);
        void *object = objects[0];
        for (size_t remaining = ROOT_COUNT; remaining != 0; remaining--) {
            if (((ScoopObjectHeader *)object)->td == &node_td) {
                BatchNode *node = object;
                assert(node->value == remaining - 1 && node->number->value == node->value);
                object = node->next;
            } else {
                ReorderedNode *node = object;
                assert(node->header.td == &reordered_td);
                assert(node->value == remaining - 1 && node->number->value == node->value);
                object = node->next;
            }
        }
        assert(object == NULL);
    }
    scoop_rt_pop_native_roots(&roots);
    /* Unlike an empty collection, shutdown must release the last used capacity too. */
    scoop_gc_shutdown();
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        assert(scoop_gc_marker.workers[index].spare_live == NULL);
    }
    scoop_thread_leave_managed();
    scoop_thread_detach_main();
}

int main(void) {
    alarm(40);
    for (size_t workers = 1; workers <= 4; workers *= 4) {
        for (unsigned bounded = 0; bounded < 2; bounded++) {
            pid_t child = fork();
            assert(child >= 0);
            if (child == 0) {
                alarm(20);
                run_case(workers, bounded != 0);
                _exit(0);
            }
            int status = 0;
            assert(waitpid(child, &status, 0) == child);
            assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
        }
    }
    puts("live chunk reuse preserves exact scans, relocation, shrinking and shutdown");
}
