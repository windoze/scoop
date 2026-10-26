/* Coordinator lifecycle for one shared serial/parallel marking implementation. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "mark_internal.h"

static void resize_blocks(ScoopGcMarkWorker *worker, size_t retained, size_t count) {
    if (count > worker->block_capacity) {
        if (count > SIZE_MAX / sizeof *worker->blocks) {
            heap_fatal("mark block counters overflow");
        }
        ScoopGcMarkBlockCount *grown = realloc(worker->blocks, count * sizeof *grown);
        if (grown == NULL) {
            heap_fatal("out of memory preparing marker block counts");
        }
        worker->blocks = grown;
        worker->block_capacity = count;
    }
    if (count != retained) {
        memset(worker->blocks + retained, 0, (count - retained) * sizeof *worker->blocks);
    }
}

static void reset_work(ScoopGcMarkWorker *worker) {
    worker->marked = worker->traced = worker->root_slots = worker->old_slots = 0;
    worker->reference_slots = worker->tasks = worker->array_tasks = worker->stolen_tasks = 0;
    worker->dirty_cards = 0;
    worker->cpu_ns = 0;
}

void scoop_gc_mark_begin(bool minor, size_t block_count) {
    scoop_gc_mark_pool_prepare(minor);
    if (atomic_load_explicit(&scoop_gc_marker.pending, memory_order_relaxed) != 0) {
        heap_fatal("mark work survived the prior collection");
    }
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        if (worker->live != NULL || worker->queue.count != 0 || worker->pending_count != 0) {
            heap_fatal("mark state was not retired by the prior collection");
        }
        resize_blocks(worker, 0, block_count);
        worker->minor = minor;
        reset_work(worker);
    }
    scoop_gc_marker.marked = 0;
    /* Root publication holds one outstanding producer until its partial batch is flushed. */
    atomic_store_explicit(&scoop_gc_marker.pending, 1, memory_order_relaxed);
}

static void scan_region(void *base, const uint64_t *scan, void *context) {
    scoop_gc_scan_descriptor(base, scan, scoop_gc_mark_slot, context, 0, UINTPTR_MAX);
}

void scoop_gc_mark_roots(void) {
    ScoopGcMarkVisit visit = {&scoop_gc_marker.workers[0], SCOOP_MARK_ROOT};
    scoop_gc_scan_roots((ScoopGcRootVisitor){
        .visit_slot = scoop_gc_mark_slot,
        .visit_region = scan_region,
        .context = &visit,
    });
}

typedef struct RememberedVisit {
    ScoopGcMarkVisit visit;
    void *last_object;
} RememberedVisit;

static void scan_dirty(void *object, uintptr_t begin, uintptr_t end, void *context) {
    RememberedVisit *visit = context;
    if (visit->last_object != object) {
        scoop_gc_validate_object(object, scoop_gc_object_size_locked(object));
        visit->last_object = object;
    }
    const uint64_t *scan = scoop_gc_object_scan(object);
    scoop_gc_scan_descriptor(object, scan, scoop_gc_mark_slot, &visit->visit, begin, end);
}

void scoop_gc_mark_remembered(void) {
    RememberedVisit visit = {.visit = {&scoop_gc_marker.workers[0], SCOOP_MARK_REMEMBERED}};
    scoop_gc_scan_remembered(scan_dirty, &visit, true);
}

void scoop_gc_mark_dirty_groups(void) {
    ScoopGcCardGroup *group = atomic_exchange_explicit(&scoop_gc_heap_state.dirty_card_groups,
                                                       NULL, memory_order_acquire);
    ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[0];
    while (group != NULL) {
        ScoopGcCardGroup *next = scoop_heap_next_dirty_card_group(group);
        /* Queued groups stay claimed; intervening writes are read when the task runs. */
        atomic_store_explicit(&group->next, group, memory_order_relaxed);
        scoop_gc_mark_queue_push(worker, (ScoopGcMarkTask){
            .kind = SCOOP_MARK_DIRTY_TASK,
            .data.dirty_group = group,
        });
        group = next;
    }
}

typedef struct DirtyVisit {
    ScoopGcMarkVisit visit;
    ScoopGcBlockMeta *block;
    void *last_object;
    const uint64_t *scan;
} DirtyVisit;

static void scan_dirty_live(void *object, uintptr_t begin, uintptr_t end, void *context) {
    DirtyVisit *visit = context;
    if (visit->last_object != object) {
        const ScoopGcBlockMeta *block = visit->block;
        size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
        bool live = block->kind == SCOOP_BLOCK_KIND_LARGE
                        ? __atomic_load_n(&block->large_marked, __ATOMIC_RELAXED)
                        : (__atomic_load_n(&block->marks[word / 64], __ATOMIC_RELAXED) &
                           (UINT64_C(1) << (word % 64))) != 0;
        /* A white owner discovered by another worker will scan all its current fields. */
        visit->scan = live ? scoop_gc_object_scan(object) : NULL;
        visit->last_object = object;
    }
    if (visit->scan != NULL) {
        scoop_gc_scan_descriptor(object, visit->scan, scoop_gc_mark_slot, &visit->visit, begin, end);
    }
}

void scoop_gc_mark_run_dirty(ScoopGcMarkWorker *worker, ScoopGcCardGroup *group) {
    ScoopGcRegion *region = group->region;
    size_t index = region->large ? 0 : (size_t)(group - region->card_groups);
    DirtyVisit visit = {
        .visit = {worker, SCOOP_MARK_OBJECT},
        .block = &region->blocks[index],
    };
    worker->dirty_cards += scoop_gc_scan_dirty_group(group, scan_dirty_live, &visit, false);
    scoop_heap_clear_dirty_card_group(group);
}

void scoop_gc_mark_publish(void) {
    scoop_gc_mark_flush(&scoop_gc_marker.workers[0]);
    scoop_gc_mark_task_done();
}

static void record_work(void) {
    ScoopGcMetrics *metrics = &scoop_gc_heap_state.metrics;
    metrics->last_mark_workers = scoop_gc_marker.active_workers;
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        scoop_gc_marker.marked += worker->marked;
        metrics->traced_objects += worker->traced;
        metrics->root_slots += worker->root_slots;
        metrics->old_reference_slots += worker->old_slots;
        metrics->dirty_cards += worker->dirty_cards;
        metrics->mark_reference_slots += worker->reference_slots;
        metrics->mark_tasks += worker->tasks;
        metrics->array_tasks += worker->array_tasks;
        metrics->stolen_tasks += worker->stolen_tasks;
        metrics->worker_cpu_ns[index] += worker->cpu_ns;
        metrics->worker_marked_objects[index] += worker->marked;
        reset_work(worker);
    }
}

bool scoop_gc_mark_step(bool finish) {
    scoop_gc_mark_pool_run(finish ? 0 : 1024);
    record_work();
    return atomic_load_explicit(&scoop_gc_marker.pending, memory_order_relaxed) == 0;
}

void scoop_gc_mark_resume(size_t retained_blocks, size_t total_blocks) {
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        resize_blocks(worker, retained_blocks, total_blocks);
    }
    /* The new root/dirty-card producer joins any work left by a bounded slice. */
    atomic_fetch_add_explicit(&scoop_gc_marker.pending, 1, memory_order_relaxed);
}

uint64_t scoop_gc_mark_finish(bool minor) {
    scoop_gc_mark_publish();
    scoop_gc_mark_pool_run(0);
    record_work();
    return scoop_gc_mark_complete(minor);
}

uint64_t scoop_gc_mark_complete(bool minor) {
    scoop_gc_heap_state.metrics.parallel_collections += scoop_gc_marker.active_workers > 1;
    for (ScoopGcBlockMeta *block = scoop_heap_first_collection_block(minor); block != NULL;
         block = scoop_heap_next_collection_block(block, minor)) {
        if (!active_head(block)) {
            continue;
        }
        for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
            ScoopGcMarkBlockCount counts = scoop_gc_marker.workers[index].blocks[block->mark_index];
            block->live_bytes += counts.live_bytes;
            block->movable_live_bytes += counts.movable_bytes;
        }
    }
    return scoop_gc_marker.marked;
}

void scoop_gc_mark_visit_live(ScoopGcLiveEntryVisitor visitor, void *context) {
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        for (ScoopGcLiveChunk *chunk = scoop_gc_marker.workers[index].live; chunk != NULL;
             chunk = chunk->next) {
            for (size_t entry = 0; entry < chunk->used; entry++) {
                visitor(&chunk->objects[entry], context);
            }
        }
    }
}

void scoop_gc_mark_dispose(void) {
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        for (size_t offset = 0; offset < worker->queue.count; offset++) {
            size_t slot = (worker->queue.first + offset) % worker->queue.capacity;
            ScoopGcMarkTask task = worker->queue.tasks[slot];
            if (task.kind == SCOOP_MARK_DIRTY_TASK) {
                scoop_heap_clear_dirty_card_group(task.data.dirty_group);
            }
        }
        while (worker->live != NULL) {
            ScoopGcLiveChunk *next = worker->live->next;
            free(worker->live);
            worker->live = next;
        }
        worker->tail = NULL;
        worker->queue.first = worker->queue.count = 0;
        worker->pending_objects = NULL;
        worker->pending_count = 0;
        worker->pending_scans = 0;
    }
    atomic_store_explicit(&scoop_gc_marker.pending, 0, memory_order_relaxed);
}
