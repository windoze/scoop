/* Collection measurements and existing heap diagnostics. */
#include <inttypes.h>
#include <stdio.h>

#include "gc_internal.h"
#include "heap_internal.h"

uint64_t scoop_rt_gc_stats(void) {
    return atomic_load_explicit(&live_objects, memory_order_acquire);
}

uint64_t scoop_rt_gc_debug_last_moved_count(void) {
    return atomic_load_explicit(&last_moved_objects, memory_order_acquire);
}

uint64_t scoop_rt_gc_debug_block_count(void) {
    lock_heap();
    uint64_t count = active_block_heads;
    unlock_heap();
    return count;
}

uintptr_t scoop_rt_gc_debug_arena_base(void) { return arena_base; }

bool scoop_rt_gc_debug_is_allocated(const void *object) {
    lock_heap();
    bool allocated = scoop_gc_is_object_start_locked(object);
    unlock_heap();
    return allocated;
}

uint64_t scoop_rt_gc_debug_allocation_size(const void *object) {
    lock_heap();
    uint64_t size = (uint64_t)scoop_gc_object_size_locked(object);
    unlock_heap();
    return size;
}

void scoop_rt_gc_debug_metrics(ScoopGcMetrics *result) {
    lock_heap();
    *result = scoop_gc_heap_state.metrics;
    result->allocated_bytes =
        atomic_load_explicit(&scoop_gc_heap_state.allocated_bytes, memory_order_relaxed);
    result->nursery_allocated_bytes =
        atomic_load_explicit(&scoop_gc_heap_state.nursery_allocated_bytes, memory_order_relaxed);
    result->heap_committed_bytes = committed_bytes;
    unlock_heap();
}

void scoop_gc_report_metrics(void) {
    if (!scoop_gc_heap_state.print_metrics) {
        return;
    }
    ScoopGcMetrics result;
    scoop_rt_gc_debug_metrics(&result);
    fprintf(stderr,
            "{\"scoop_gc\":1,\"minor_collections\":%" PRIu64 ",\"full_collections\":%" PRIu64
            ",\"promotion_fallbacks\":%" PRIu64 ",\"allocated_bytes\":%" PRIu64
            ",\"nursery_allocated_bytes\":%" PRIu64 ",\"promoted_bytes\":%" PRIu64
            ",\"dirty_cards\":%" PRIu64 ",\"old_reference_slots\":%" PRIu64
            ",\"root_slots\":%" PRIu64 ",\"traced_objects\":%" PRIu64 ",\"pause_ns\":%" PRIu64
            ",\"maximum_pause_ns\":%" PRIu64 ",\"committed_bytes\":%" PRIu64 "}\n",
            result.minor_collections, result.full_collections, result.promotion_fallbacks,
            result.allocated_bytes, result.nursery_allocated_bytes, result.promoted_bytes,
            result.dirty_cards, result.old_reference_slots, result.root_slots,
            result.traced_objects, result.pause_ns, result.maximum_pause_ns,
            result.heap_committed_bytes);
}
