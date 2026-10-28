/* STW-only readers and atomic first marking; workers own all counters and lists. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "../gc_binding/scan.h"
#include "mark_internal.h"
#include "object_metadata.h"

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

static __attribute__((cold)) void extend_live(ScoopGcMarkWorker *worker) {
    scoop_gc_mark_flush(worker);
    ScoopGcLiveChunk *chunk = worker->spare_live;
    if (chunk != NULL) {
        worker->spare_live = chunk->next;
    } else {
        chunk = malloc(sizeof *chunk);
        if (chunk == NULL) {
            heap_fatal("out of memory retaining the marked object set");
        }
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

/* Keep the per-object append in its marker; chunk acquisition is shared. */
static inline __attribute__((always_inline)) void
append_live(ScoopGcMarkWorker *worker, void *object, bool scan) {
    if (worker->tail == NULL || worker->tail->used == GC_MARK_CHUNK) {
        extend_live(worker);
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
    if (block->head->kind == SCOOP_BLOCK_KIND_LARGE) {
        return !__atomic_exchange_n(&block->large_marked, true, __ATOMIC_RELAXED);
    }
    uint64_t mask = UINT64_C(1) << (word % 64);
    uint64_t *bits = &block->marks[word / 64];
    if (__atomic_load_n(bits, __ATOMIC_RELAXED) & mask) {
        return false;
    }
    return (__atomic_fetch_or(bits, mask, __ATOMIC_RELAXED) & mask) == 0;
}

static void mark_lines(ScoopGcMarkBlockCount *counts, size_t word, size_t size) {
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
        counts->live_lines[index] |= bits;
    }
}

static bool slot_aligned(size_t word, size_t size) {
    size_t offset = word * sizeof(uint64_t) - GC_LINE_SIZE;
    size_t mask = size - 1;
    return (size & mask) == 0 ? (offset & mask) == 0 : offset % size == 0;
}

static void mark_slot_layout(ScoopGcMarkBlockCount *counts, size_t word, size_t size) {
    if (counts->slot_size == SIZE_MAX) {
        return;
    }
    if (size > GC_OLD_SLOT_MAX || (counts->slot_size != 0 && counts->slot_size != size) ||
        !slot_aligned(word, size)) {
        counts->slot_size = SIZE_MAX;
    } else {
        counts->slot_size = size;
    }
}

static void enqueue_array(ScoopGcMarkWorker *worker, void *object, const uint64_t *scan) {
    ScoopGcArrayScan array = scoop_gc_array_scan(object, scan);
    for (uint64_t first = 0; first < array.count;) {
        uint64_t length = array.count - first;
        if (length > GC_MARK_ARRAY_RANGE) {
            length = GC_MARK_ARRAY_RANGE;
        }
        uint64_t stride = scoop_gc_array_stride(scan);
        scoop_gc_mark_queue_push(worker,
                                 (ScoopGcMarkTask){
                                     .kind = SCOOP_MARK_ARRAY_TASK,
                                     .data.range = {object, scan, array.elements + first * stride,
                                                    array.elements + (first + length) * stride},
                                 });
        first += length;
    }
}

static ScoopGcRegion *reference_region(ScoopGcMarkWorker *worker, const void *object) {
    uintptr_t address = (uintptr_t)object;
    uintptr_t base = address & ~(uintptr_t)(GC_REGION_SIZE - 1);
    ScoopGcMarkRegion *entry = &worker->regions[(address / GC_REGION_SIZE) % GC_MARK_REGION_CACHE];
    if (entry->region != NULL && entry->base == base) {
        return entry->region;
    }
    ScoopGcRegion *region = scoop_heap_region_for_address(address);
    if (region != NULL && !region->large) {
        *entry = (ScoopGcMarkRegion){base, region};
    }
    return region;
}

static inline __attribute__((always_inline)) ScoopGcBlockMeta *
markable_block(ScoopGcMarkWorker *worker, const void *object, size_t *word) {
    ScoopGcRegion *region = reference_region(worker, object);
    /* The coordinator owns heap/roots; these metadata remain immutable during mark. */
    if (region == NULL) {
#if SCOOP_ENABLE_GC_STRESS || (defined(SCOOP_GC_VERIFY_HEAP) && SCOOP_GC_VERIFY_HEAP)
        if (!scoop_gc_stw_is_stable_object(object)) {
            heap_fatal("managed slot points outside the current heap and stable object tables");
        }
#endif
        return NULL;
    }
    size_t index = 0;
    if (region->large) {
        if (!scoop_heap_object_meta_in_block(object, &region->blocks[0], word)) {
            heap_fatal("managed slot is not a current heap object start");
        }
    } else {
        uintptr_t offset = (uintptr_t)object - region->base;
        index = offset / GC_BLOCK_SIZE;
        if (!scoop_heap_small_object_meta_at(offset % GC_BLOCK_SIZE, &region->heads[index],
                                             &region->starts[index * GC_BITMAP_WORDS], word)) {
            heap_fatal("managed slot is not a current heap object start");
        }
    }
    if (worker->minor && region->heads[index].generation != SCOOP_GC_YOUNG) {
        return NULL;
    }
    return &region->blocks[index];
}

static void record_remembered_slot(ScoopGcMarkWorker *worker, void **slot) {
    if (!worker->remembered_complete) {
        return;
    }
    ScoopGcRememberedGroup *group = worker->remembered_current;
    if (group == NULL) {
        group = malloc(sizeof *group);
        if (group == NULL) {
            worker->remembered_complete = false;
            return;
        }
        group->next = worker->remembered_slots;
        group->base = (uintptr_t)slot & ~(GC_REMEMBERED_GROUP_BYTES - 1);
        memset(group->slots, 0, sizeof group->slots);
        worker->remembered_slots = worker->remembered_current = group;
    }
    size_t word = ((uintptr_t)slot & (GC_REMEMBERED_GROUP_BYTES - 1)) / sizeof *slot;
    group->slots[word / 64] |= UINT64_C(1) << (word % 64);
}

static inline __attribute__((always_inline)) void
mark_target(ScoopGcMarkWorker *worker, void *object, void **remembered_slot) {
    size_t word;
    ScoopGcBlockMeta *block = markable_block(worker, object, &word);
    if (block == NULL) {
        return;
    }
    if (remembered_slot != NULL) {
        record_remembered_slot(worker, remembered_slot);
    }
    if (!first_mark(block, word)) {
        return;
    }
    size_t size = block->head->kind == SCOOP_BLOCK_KIND_LARGE
                      ? block->exact_size
                      : scoop_heap_small_object_size(object);
    scoop_gc_validate_object(object, size);
    ScoopGcMarkBlockCount *counts = &worker->blocks[block->mark_index];
    if (block->head->kind == SCOOP_BLOCK_KIND_SMALL) {
        mark_lines(counts, word, size);
    }
    counts->live_bytes += size;
    if (worker->slot_layout) {
        mark_slot_layout(counts, word, size);
    } else if (!object_pinned(block, word)) {
        counts->movable_bytes += size;
    }
    worker->marked++;
    worker->traced++;
    const uint64_t *scan = scoop_gc_object_scan(object);
    bool array = scan != NULL && scoop_gc_scan_is_array(scan);
    append_live(worker, object, scan != NULL && !array);
    if (array) {
        enqueue_array(worker, object, scan);
    }
}

/* Known slot loops inline this body; generic scanners retain its external entry. */
extern inline __attribute__((always_inline)) void scoop_gc_mark_slot(void **slot, void *context) {
    ScoopGcMarkVisit *visit = context;
    if (visit->source == SCOOP_MARK_ROOT) {
        visit->worker->root_slots++;
    } else if (visit->source == SCOOP_MARK_REMEMBERED) {
        visit->worker->old_slots++;
    } else {
        visit->worker->reference_slots++;
    }
    if (*slot != NULL) {
        mark_target(visit->worker, *slot,
                    visit->source == SCOOP_MARK_REMEMBERED ? slot : NULL);
    }
}

void scoop_gc_mark_range(void *object, const uint64_t *scan, ScoopGcMarkVisit *visit,
                         uintptr_t begin, uintptr_t end) {
    scoop_gc_scan_range(object, scan, scoop_gc_mark_slot, visit, begin, end);
}

void scoop_gc_mark_run_task(ScoopGcMarkWorker *worker, ScoopGcMarkTask task) {
    ScoopGcMarkVisit visit = {worker, SCOOP_MARK_OBJECT};
    worker->tasks++;
    if (task.kind == SCOOP_MARK_DIRTY_TASK) {
        scoop_gc_mark_run_dirty(worker, task.data.dirty.group, task.data.dirty.saved);
        return;
    }
    if (task.kind == SCOOP_MARK_ARRAY_TASK) {
        worker->array_tasks++;
        scoop_gc_mark_range(task.data.range.object, task.data.range.scan, &visit,
                             task.data.range.begin, task.data.range.end);
        return;
    }
    uint64_t scans = task.data.batch.scans;
    size_t scanned = 0;
    while (scans != 0) {
        size_t index = (size_t)__builtin_ctzll(scans);
        void *object = task.data.batch.objects[index];
        const uint64_t *scan = scoop_gc_object_scan(object);
        scoop_gc_scan_object(object, scan, scoop_gc_mark_slot, &visit);
        scans &= scans - 1;
        scanned++;
    }
    /* Keep short continuations in this outstanding task; publish any remainder on return. */
    while (scanned < GC_MARK_BATCH && worker->pending_scans != 0) {
        size_t index = (size_t)__builtin_ctzll(worker->pending_scans);
        void *object = worker->pending_objects[index];
        worker->pending_scans &= worker->pending_scans - 1;
        const uint64_t *scan = scoop_gc_object_scan(object);
        scoop_gc_scan_object(object, scan, scoop_gc_mark_slot, &visit);
        scanned++;
    }
}
