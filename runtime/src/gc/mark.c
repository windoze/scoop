/* Coordinator lifecycle for one shared serial/parallel marking implementation. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "mark_internal.h"
#include "old_slots.h"

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
        memset(worker->regions, 0, sizeof worker->regions);
        worker->minor = minor;
        worker->slot_layout = minor && scoop_heap_promotes_slot_blocks();
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
    if (scoop_gc_marker.active_workers > 1) {
        scoop_gc_mark_enqueue_remembered();
        return;
    }
    RememberedVisit visit = {.visit = {&scoop_gc_marker.workers[0], SCOOP_MARK_REMEMBERED}};
    scoop_gc_scan_remembered(scan_dirty, &visit, true);
}

static ScoopGcSavedCards *save_dirty_group(ScoopGcCardGroup *group) {
    ScoopGcSavedCardChunk *chunk = scoop_gc_marker.saved_cards;
    if (chunk == NULL || chunk->used == GC_MARK_CHUNK) {
        chunk = malloc(sizeof *chunk);
        if (chunk == NULL) {
            return NULL;
        }
        chunk->next = scoop_gc_marker.saved_cards;
        chunk->used = 0;
        scoop_gc_marker.saved_cards = chunk;
    }
    ScoopGcSavedCards *saved = &chunk->entries[chunk->used++];
    *saved = (ScoopGcSavedCards){.group = group};
    return saved;
}

bool scoop_gc_mark_dirty_groups(bool retain_remembered, bool first) {
    ScoopGcCardGroup *group = atomic_exchange_explicit(&scoop_gc_heap_state.dirty_card_groups,
                                                       NULL, memory_order_acquire);
    ScoopGcMarkWorker *worker = &scoop_gc_marker.workers[0];
    while (group != NULL) {
        ScoopGcCardGroup *next = scoop_heap_next_dirty_card_group(group);
        if (retain_remembered && first) {
            ScoopGcRegion *region = group->region;
            size_t index = region->large ? 0 : (size_t)(group - region->card_groups);
            if (region->blocks[index].generation == SCOOP_GC_YOUNG) {
                /* The first root/remembered traversal will scan the current young graph. */
                scoop_heap_clear_dirty_card_group(group);
                group = next;
                continue;
            }
        }
        ScoopGcSavedCards *saved = retain_remembered ? save_dirty_group(group) : NULL;
        if (retain_remembered && saved == NULL) {
            /* Keep the unqueued tail registered for the complete full fallback. */
            atomic_store_explicit(&scoop_gc_heap_state.dirty_card_groups, group,
                                  memory_order_relaxed);
            return false;
        }
        /* Queued groups stay claimed; intervening writes are read when the task runs. */
        atomic_store_explicit(&group->next, group, memory_order_relaxed);
        scoop_gc_mark_queue_push(worker, (ScoopGcMarkTask){
            .kind = SCOOP_MARK_DIRTY_TASK,
            .data.dirty = {group, saved},
        });
        group = next;
    }
    return true;
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

void scoop_gc_mark_run_dirty(ScoopGcMarkWorker *worker, ScoopGcCardGroup *group,
                              ScoopGcSavedCards *saved) {
    if (worker->minor && saved == NULL) {
        RememberedVisit visit = {.visit = {worker, SCOOP_MARK_REMEMBERED}};
        worker->dirty_cards += scoop_gc_scan_dirty_group(group, scan_dirty, &visit, true);
        return;
    }
    ScoopGcRegion *region = group->region;
    size_t index = region->large ? 0 : (size_t)(group - region->card_groups);
    ScoopGcBlockMeta *block = &region->blocks[index];
    if (worker->minor && block->generation == SCOOP_GC_OLD) {
        RememberedVisit visit = {.visit = {worker, SCOOP_MARK_REMEMBERED}};
        worker->dirty_cards += scoop_gc_scan_dirty_group(group, scan_dirty, &visit, true);
        const unsigned char *cards =
            region->cards + (group - region->card_groups) * GC_CARDS_PER_GROUP;
        uint64_t bits = 0;
        for (size_t card = 0; card < GC_CARDS_PER_GROUP; card++) {
            bits |= (uint64_t)(cards[card] != 0) << card;
        }
        saved->bits = bits;
    } else {
        DirtyVisit visit = {
            .visit = {worker, SCOOP_MARK_OBJECT},
            .block = block,
        };
        worker->dirty_cards += scoop_gc_scan_dirty_group(group, scan_dirty_live, &visit, false);
    }
    scoop_heap_clear_dirty_card_group(group);
}

void scoop_gc_mark_restore_remembered(void) {
    /* Workers have finished. Merge repeated groups before assigning update ownership. */
    for (ScoopGcSavedCardChunk *chunk = scoop_gc_marker.saved_cards; chunk != NULL;
         chunk = chunk->next) {
        for (size_t entry = 0; entry < chunk->used; entry++) {
            ScoopGcSavedCards saved = chunk->entries[entry];
            ScoopGcRegion *region = saved.group->region;
            size_t first = (size_t)(saved.group - region->card_groups) * GC_CARDS_PER_GROUP;
            uint64_t bits = saved.bits;
            while (bits != 0) {
                size_t card = (size_t)__builtin_ctzll(bits);
                region->cards[first + card] = 1;
                bits &= bits - 1;
            }
            if (saved.bits != 0) {
                scoop_runtime_record_dirty_card_group(region, first);
            }
        }
    }
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
        memset(worker->regions, 0, sizeof worker->regions);
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
    bool slot_layout = scoop_gc_marker.workers[0].slot_layout;
    for (ScoopGcBlockMeta *block = scoop_heap_first_collection_block(minor); block != NULL;
         block = scoop_heap_next_collection_block(block, minor)) {
        if (!active_head(block)) {
            continue;
        }
        for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
            const ScoopGcMarkBlockCount *counts =
                &scoop_gc_marker.workers[index].blocks[block->mark_index];
            block->live_bytes += counts->live_bytes;
            if (slot_layout) {
                size_t size = counts->slot_size;
                if (block->nursery_slot_size == 0) {
                    block->nursery_slot_size = size;
                } else if (size != 0 && size != block->nursery_slot_size) {
                    block->nursery_slot_size = SIZE_MAX;
                }
            } else {
                block->movable_live_bytes += counts->movable_bytes;
            }
            if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
                for (size_t line = 0; line < GC_LINE_BITMAP_WORDS; line++) {
                    block->line_live[line] |= counts->live_lines[line];
                }
            }
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
                scoop_heap_clear_dirty_card_group(task.data.dirty.group);
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
    while (scoop_gc_marker.saved_cards != NULL) {
        ScoopGcSavedCardChunk *next = scoop_gc_marker.saved_cards->next;
        free(scoop_gc_marker.saved_cards);
        scoop_gc_marker.saved_cards = next;
    }
    atomic_store_explicit(&scoop_gc_marker.pending, 0, memory_order_relaxed);
}
