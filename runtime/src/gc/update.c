/* STW reference update: coordinator partitions; workers visit disjoint slots. */
#include "../gc_binding/object.h"
#include "mark_internal.h"

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

void scoop_gc_update_live(ScoopGcSlotVisitor visitor, ScoopGcLiveEntryVisitor serial_visitor,
                          void *context) {
    if (scoop_gc_marker.active_workers == 1) {
        /* Preserve the coordinator's direct callback without a per-object wrapper. */
        scoop_gc_mark_visit_live(serial_visitor, context);
        return;
    }
    scoop_gc_marker.updating = true;
    scoop_gc_marker.update_visitor = visitor;
    scoop_gc_marker.update_context = context;
    for (size_t index = 0; index < scoop_gc_marker.active_workers; index++) {
        ScoopGcMarkWorker *owner = &scoop_gc_marker.workers[index];
        for (ScoopGcLiveChunk *chunk = owner->live; chunk != NULL; chunk = chunk->next) {
            for (size_t first = 0; first < chunk->used; first += GC_MARK_BATCH) {
                uint64_t scans = 0;
                size_t count = chunk->used - first;
                if (count > GC_MARK_BATCH) {
                    count = GC_MARK_BATCH;
                }
                for (size_t offset = 0; offset < count; offset++) {
                    void *object = chunk->objects[first + offset];
                    const uint64_t *scan = scoop_gc_object_scan(object);
                    if (scan == NULL) {
                        continue;
                    }
#if SCOOP_GC_TRACK_RELOCATION
                    scoop_heap_claim_current_object(object);
#endif
                    if (scoop_gc_scan_is_array(scan)) {
                        enqueue_update_array(owner, object, scan);
                    } else {
                        scans |= UINT64_C(1) << offset;
                    }
                }
                if (scans != 0) {
                    scoop_gc_mark_queue_push(owner,
                                             (ScoopGcMarkTask){
                                                 .data.batch = {&chunk->objects[first], scans},
                                             });
                }
            }
        }
    }
    scoop_gc_mark_pool_run(0);
    scoop_gc_marker.updating = false;
    scoop_gc_marker.update_visitor = NULL;
    scoop_gc_marker.update_context = NULL;
}

void scoop_gc_update_run_task(ScoopGcMarkTask task) {
    ScoopGcSlotVisitor visitor = scoop_gc_marker.update_visitor;
    void *context = scoop_gc_marker.update_context;
    if (task.kind == SCOOP_MARK_ARRAY_TASK) {
        scoop_gc_scan_descriptor(task.data.range.object, task.data.range.scan, visitor, context,
                                 task.data.range.begin, task.data.range.end);
        return;
    }
    uint64_t scans = task.data.batch.scans;
    while (scans != 0) {
        size_t index = (size_t)__builtin_ctzll(scans);
        void *current = task.data.batch.objects[index];
        scoop_gc_scan_descriptor(current, scoop_gc_object_scan(current), visitor, context, 0,
                                 UINTPTR_MAX);
        scans &= scans - 1;
    }
}
