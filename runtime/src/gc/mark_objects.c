/* STW-only readers and atomic first marking; workers own all counters and lists. */
#include <stdlib.h>

#include "../value_shape.h"
#include "mark_internal.h"

void scoop_gc_mark_flush(ScoopGcMarkWorker *worker) {
    if (worker->pending_scans != 0) {
        scoop_gc_mark_queue_push(worker,
                                 (ScoopGcMarkTask){
                                     .data.batch = {worker->pending_objects, worker->pending_scans},
                                 });
    }
    worker->pending_objects = NULL;
    worker->pending_count = 0;
    worker->pending_scans = 0;
}

static void append_live(ScoopGcMarkWorker *worker, void *object, bool scan) {
    if (worker->tail == NULL || worker->tail->used == GC_MARK_CHUNK) {
        scoop_gc_mark_flush(worker);
        ScoopGcLiveChunk *chunk = malloc(sizeof *chunk);
        if (chunk == NULL) {
            heap_fatal("out of memory retaining the marked object set");
        }
        chunk->next = NULL;
        chunk->used = 0;
        if (worker->tail == NULL) {
            worker->live = chunk;
        } else {
            worker->tail->next = chunk;
        }
        worker->tail = chunk;
    }
    void **entry = &worker->tail->objects[worker->tail->used++];
    *entry = object;
    if (worker->pending_count == 0) {
        worker->pending_objects = entry;
    }
    if (scan) {
        worker->pending_scans |= UINT64_C(1) << worker->pending_count;
    }
    worker->pending_count++;
    if (worker->pending_count == GC_MARK_BATCH) {
        scoop_gc_mark_flush(worker);
    }
}

static bool first_mark(ScoopGcBlockMeta *block, size_t word) {
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        return !__atomic_exchange_n(&block->large_marked, true, __ATOMIC_RELAXED);
    }
    uint64_t mask = UINT64_C(1) << (word % 64);
    uint64_t *bits = &block->marks[word / 64];
    if (__atomic_load_n(bits, __ATOMIC_RELAXED) & mask) {
        return false;
    }
    return (__atomic_fetch_or(bits, mask, __ATOMIC_RELAXED) & mask) == 0;
}

static void mark_lines(ScoopGcBlockMeta *block, size_t word, size_t size) {
    size_t first = word * sizeof(uint64_t) / GC_LINE_SIZE;
    size_t last = (word * sizeof(uint64_t) + size - 1) / GC_LINE_SIZE;
    for (size_t index = first / 64; index <= last / 64; index++) {
        uint64_t bits = UINT64_MAX;
        if (index == first / 64) {
            bits &= UINT64_MAX << (first % 64);
        }
        if (index == last / 64) {
            bits &= UINT64_MAX >> (63 - last % 64);
        }
        __atomic_fetch_or(&block->line_live[index], bits, __ATOMIC_RELAXED);
    }
}

static void enqueue_array(ScoopGcMarkWorker *worker, void *object, const uint64_t *scan) {
    uint64_t count = *(const uint64_t *)((char *)object + scan[1]);
    uintptr_t elements = (uintptr_t)object + scan[2];
    for (uint64_t first = 0; first < count;) {
        uint64_t length = count - first;
        if (length > GC_MARK_ARRAY_RANGE) {
            length = GC_MARK_ARRAY_RANGE;
        }
        scoop_gc_mark_queue_push(worker,
                                 (ScoopGcMarkTask){
                                     .array = true,
                                     .data.range = {object, scan, elements + first * scan[3],
                                                    elements + (first + length) * scan[3]},
                                 });
        first += length;
    }
}

static void mark_target(ScoopGcMarkWorker *worker, void *object) {
    ScoopGcBlockMeta *block;
    size_t word;
    /* The coordinator owns heap/roots; these metadata remain immutable during mark. */
    if (!object_meta(object, &block, &word)) {
        if (!scoop_gc_stw_is_stable_object(object)) {
            heap_fatal("managed slot points outside the current heap and stable object tables");
        }
        return;
    }
    if ((worker->minor && block->generation != SCOOP_GC_YOUNG) || !first_mark(block, word)) {
        return;
    }
    size_t size = block->kind == SCOOP_BLOCK_KIND_LARGE
                      ? block->exact_size
                      : (size_t)block->size_units[word] * sizeof(uint64_t);
    scoop_shape_validate_object(object, size);
    if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
        mark_lines(block, word, size);
    }
    ScoopGcMarkBlockCount *counts = &worker->blocks[block->mark_index];
    counts->live_bytes += size;
    if (!object_pinned(block, word)) {
        counts->movable_bytes += size;
    }
    worker->marked++;
    worker->traced++;
    const uint64_t *scan = ((const ScoopObjectHeader *)object)->td->object_scan;
    bool array = scan != NULL && scan[0] == SCOOP_REFS_ARRAY;
    append_live(worker, object, scan != NULL && !array);
    if (array) {
        enqueue_array(worker, object, scan);
    }
}

void scoop_gc_mark_slot(void **slot, void *context) {
    ScoopGcMarkVisit *visit = context;
    if (visit->source == SCOOP_MARK_ROOT) {
        visit->worker->root_slots++;
    } else if (visit->source == SCOOP_MARK_REMEMBERED) {
        visit->worker->old_slots++;
    } else {
        visit->worker->reference_slots++;
    }
    if (*slot != NULL) {
        mark_target(visit->worker, *slot);
    }
}

void scoop_gc_mark_run_task(ScoopGcMarkWorker *worker, ScoopGcMarkTask task) {
    ScoopGcMarkVisit visit = {worker, SCOOP_MARK_OBJECT};
    worker->tasks++;
    if (task.array) {
        worker->array_tasks++;
        scoop_gc_scan_descriptor(task.data.range.object, task.data.range.scan, scoop_gc_mark_slot,
                                 &visit, task.data.range.begin, task.data.range.end);
        return;
    }
    uint64_t scans = task.data.batch.scans;
    size_t scanned = 0;
    while (scans != 0) {
        size_t index = (size_t)__builtin_ctzll(scans);
        void *object = task.data.batch.objects[index];
        const uint64_t *scan = ((const ScoopObjectHeader *)object)->td->object_scan;
        scoop_gc_scan_descriptor(object, scan, scoop_gc_mark_slot, &visit, 0, UINTPTR_MAX);
        scans &= scans - 1;
        scanned++;
    }
    /* Keep short continuations in this outstanding task; publish any remainder on return. */
    while (scanned < GC_MARK_BATCH && worker->pending_scans != 0) {
        size_t index = (size_t)__builtin_ctzll(worker->pending_scans);
        void *object = worker->pending_objects[index];
        worker->pending_scans &= worker->pending_scans - 1;
        const uint64_t *scan = ((const ScoopObjectHeader *)object)->td->object_scan;
        scoop_gc_scan_descriptor(object, scan, scoop_gc_mark_slot, &visit, 0, UINTPTR_MAX);
        scanned++;
    }
}
