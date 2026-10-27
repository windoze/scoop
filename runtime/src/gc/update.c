/* STW reference update: coordinator partitions; workers visit disjoint slots. */
#include "../gc_binding/object.h"
#include "mark_internal.h"

typedef struct UpdateDirtyVisit {
    ScoopGcSlotVisitor visitor;
    void *context;
} UpdateDirtyVisit;

static void update_dirty_object(void *object, uintptr_t begin, uintptr_t end, void *context) {
    UpdateDirtyVisit *visit = context;
    scoop_gc_scan_descriptor(object, scoop_gc_object_scan(object), visit->visitor, visit->context,
                             begin, end);
}

static void update_old_slot_object(void *object, uintptr_t begin, uintptr_t end, void *context) {
    bool small = scoop_heap_small_object_size(object) != 0;
    ScoopGcBlockMeta *block = scoop_heap_known_object_block(object, small);
    size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
    if (small && block->slot_size != 0 && bit_test(block->marks, word)) {
        /* New slot occupants are updated once through the young live set. */
        return;
    }
    update_dirty_object(object, begin, end, context);
}

static ScoopGcObjectRangeVisitor dirty_visitor(void) {
    return scoop_gc_heap_state.old_slots_enabled ? update_old_slot_object : update_dirty_object;
}

static void enqueue_update_array(ScoopGcMarkWorker *owner, void *object, const uint64_t *scan) {
    ScoopGcArrayScan array = scoop_gc_array_scan(object, scan);
    uint64_t stride = scoop_gc_array_stride(scan);
    for (uint64_t first = 0; first < array.count;) {
        uint64_t length = array.count - first;
        if (length > GC_MARK_ARRAY_RANGE) {
            length = GC_MARK_ARRAY_RANGE;
        }
        scoop_gc_mark_queue_push(owner,
                                 (ScoopGcMarkTask){
                                     .kind = SCOOP_MARK_ARRAY_TASK,
                                     .data.range = {object, scan, array.elements + first * stride,
                                                    array.elements + (first + length) * stride},
                                 });
        first += length;
    }
}

void scoop_gc_update_live(bool minor, ScoopGcSlotVisitor visitor,
                          ScoopGcLiveEntryVisitor serial_visitor, void *context) {
    if (scoop_gc_marker.active_workers == 1) {
        if (minor) {
            UpdateDirtyVisit visit = {visitor, context};
            scoop_gc_scan_remembered(dirty_visitor(), &visit, false);
        }
        /* Preserve the coordinator's direct callback without a per-object wrapper. */
        scoop_gc_mark_visit_live(serial_visitor, context);
        return;
    }
    scoop_gc_marker.updating = true;
    scoop_gc_marker.update_visitor = visitor;
    scoop_gc_marker.update_context = context;
    if (minor) {
        scoop_gc_mark_enqueue_remembered();
    }
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        ScoopGcMarkWorker *owner = &scoop_gc_marker.workers[index];
        for (ScoopGcLiveChunk *chunk = owner->live; chunk != NULL; chunk = chunk->next) {
            scoop_gc_mark_queue_push(owner, (ScoopGcMarkTask){
                .kind = SCOOP_UPDATE_CHUNK_TASK,
                .data.live_chunk = chunk,
            });
        }
    }
    scoop_gc_mark_pool_run(0);
    scoop_gc_marker.updating = false;
    scoop_gc_marker.update_visitor = NULL;
    scoop_gc_marker.update_context = NULL;
}

void scoop_gc_update_run_task(ScoopGcMarkWorker *worker, ScoopGcMarkTask task) {
    ScoopGcSlotVisitor visitor = scoop_gc_marker.update_visitor;
    void *context = scoop_gc_marker.update_context;
    if (task.kind == SCOOP_MARK_DIRTY_TASK) {
        UpdateDirtyVisit visit = {visitor, context};
        (void)scoop_gc_scan_dirty_group(task.data.dirty_group, dirty_visitor(), &visit, true);
        return;
    }
    if (task.kind == SCOOP_MARK_ARRAY_TASK) {
        scoop_gc_scan_descriptor(task.data.range.object, task.data.range.scan, visitor, context,
                                 task.data.range.begin, task.data.range.end);
        return;
    }
    ScoopGcLiveChunk *chunk = task.data.live_chunk;
    for (size_t index = 0; index < chunk->used; index++) {
        void *current = chunk->objects[index];
        const uint64_t *scan = scoop_gc_object_scan(current);
        if (scan == NULL) {
            continue;
        }
#if SCOOP_GC_TRACK_RELOCATION
        scoop_heap_claim_current_object(current);
#endif
        if (scoop_gc_scan_is_array(scan)) {
            enqueue_update_array(worker, current, scan);
        } else {
            scoop_gc_scan_descriptor(current, scan, visitor, context, 0, UINTPTR_MAX);
        }
    }
}
