/* Coordinator lifecycle for one shared serial/parallel marking implementation. */
#include <stdlib.h>
#include <string.h>

#include "../value_shape.h"
#include "mark_internal.h"

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
        if (block_count > worker->block_capacity) {
            if (block_count > SIZE_MAX / sizeof *worker->blocks) {
                heap_fatal("mark block counters overflow");
            }
            ScoopGcMarkBlockCount *grown = realloc(worker->blocks, block_count * sizeof *grown);
            if (grown == NULL) {
                heap_fatal("out of memory preparing marker block counts");
            }
            worker->blocks = grown;
            worker->block_capacity = block_count;
        }
        if (block_count != 0) {
            memset(worker->blocks, 0, block_count * sizeof *worker->blocks);
        }
        worker->minor = minor;
        worker->marked = worker->traced = worker->root_slots = worker->old_slots = 0;
        worker->reference_slots = worker->tasks = worker->array_tasks = worker->stolen_tasks = 0;
        worker->cpu_ns = 0;
    }
    /* Root publication holds one outstanding producer until its partial batch is flushed. */
    atomic_store_explicit(&scoop_gc_marker.pending, 1, memory_order_relaxed);
}

static void scan_external(const void *object, void *context) {
    ScoopGcMarkVisit *visit = context;
    visit->worker->traced++;
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    scoop_gc_scan_descriptor((void *)object, td->object_scan, scoop_gc_mark_slot, context, 0,
                             UINTPTR_MAX);
}

static void scan_region(void *base, const uint64_t *scan, void *context) {
    scoop_gc_scan_descriptor(base, scan, scoop_gc_mark_slot, context, 0, UINTPTR_MAX);
}

void scoop_gc_mark_roots(void) {
    ScoopGcMarkVisit visit = {&scoop_gc_marker.workers[0], SCOOP_MARK_ROOT};
    scoop_gc_scan_roots((ScoopGcRootVisitor){
        .visit_slot = scoop_gc_mark_slot,
        .visit_external_object = scan_external,
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
        scoop_shape_validate_object(object, scoop_gc_object_size_locked(object));
        visit->last_object = object;
    }
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    scoop_gc_scan_descriptor(object, td->object_scan, scoop_gc_mark_slot, &visit->visit, begin,
                             end);
}

void scoop_gc_mark_remembered(void) {
    RememberedVisit visit = {.visit = {&scoop_gc_marker.workers[0], SCOOP_MARK_REMEMBERED}};
    scoop_gc_scan_remembered(scan_dirty, &visit, true);
}

uint64_t scoop_gc_mark_finish(bool minor) {
    scoop_gc_mark_flush(&scoop_gc_marker.workers[0]);
    scoop_gc_mark_task_done();
    scoop_gc_mark_pool_run();
    ScoopGcMetrics *metrics = &scoop_gc_heap_state.metrics;
    metrics->last_mark_workers = scoop_gc_marker.active_workers;
    metrics->parallel_collections += scoop_gc_marker.active_workers > 1;
    uint64_t marked = 0;
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        const ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        marked += worker->marked;
        metrics->traced_objects += worker->traced;
        metrics->root_slots += worker->root_slots;
        metrics->old_reference_slots += worker->old_slots;
        metrics->mark_reference_slots += worker->reference_slots;
        metrics->mark_tasks += worker->tasks;
        metrics->array_tasks += worker->array_tasks;
        metrics->stolen_tasks += worker->stolen_tasks;
        metrics->worker_cpu_ns[index] += worker->cpu_ns;
        metrics->worker_marked_objects[index] += worker->marked;
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || (minor && block->generation != SCOOP_GC_YOUNG)) {
            continue;
        }
        for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
            ScoopGcMarkBlockCount counts = scoop_gc_marker.workers[index].blocks[block->mark_index];
            block->live_bytes += counts.live_bytes;
            block->movable_live_bytes += counts.movable_bytes;
        }
    }
    return marked;
}

void scoop_gc_mark_visit_live(ScoopGcHeapObjectVisitor visitor, void *context) {
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        for (ScoopGcLiveChunk *chunk = scoop_gc_marker.workers[index].live; chunk != NULL;
             chunk = chunk->next) {
            for (size_t entry = 0; entry < chunk->used; entry++) {
                visitor(chunk->objects[entry], context);
            }
        }
    }
}

void scoop_gc_mark_dispose(void) {
    for (size_t index = 0; index < GC_MARK_MAX_WORKERS; index++) {
        ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[index];
        while (worker->live != NULL) {
            ScoopGcLiveChunk *next = worker->live->next;
            free(worker->live);
            worker->live = next;
        }
        worker->tail = NULL;
    }
}
