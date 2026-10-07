/* Collection measurements and existing heap diagnostics. */
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

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

static void metrics_locked(ScoopGcMetrics *result) {
    *result = scoop_gc_heap_state.metrics;
    result->allocated_bytes =
        atomic_load_explicit(&scoop_gc_heap_state.allocated_bytes, memory_order_relaxed);
    result->nursery_allocated_bytes =
        atomic_load_explicit(&scoop_gc_heap_state.nursery_allocated_bytes, memory_order_relaxed);
    result->heap_committed_bytes = committed_bytes;
}

void scoop_rt_gc_debug_metrics(ScoopGcMetrics *result) {
    lock_heap();
    metrics_locked(result);
    unlock_heap();
}

void scoop_gc_report_metrics(void) {
    if (!scoop_gc_heap_state.print_metrics) {
        return;
    }
    ScoopGcMetrics result;
    uint64_t buckets[8];
    lock_heap();
    metrics_locked(&result);
    uint64_t copied = scoop_gc_heap_state.copied_bytes;
    uint64_t minor_pause = scoop_gc_heap_state.minor_pause_ns;
    uint64_t full_pause = scoop_gc_heap_state.full_pause_ns;
    memcpy(buckets, scoop_gc_heap_state.pause_buckets, sizeof buckets);
    unlock_heap();
    fprintf(stderr,
            "{\"scoop_gc\":1,\"minor_collections\":%" PRIu64 ",\"full_collections\":%" PRIu64
            ",\"promotion_fallbacks\":%" PRIu64 ",\"allocated_bytes\":%" PRIu64
            ",\"nursery_allocated_bytes\":%" PRIu64 ",\"promoted_bytes\":%" PRIu64
            ",\"copied_bytes\":%" PRIu64 ",\"dirty_cards\":%" PRIu64
            ",\"old_reference_slots\":%" PRIu64 ",\"root_slots\":%" PRIu64
            ",\"traced_objects\":%" PRIu64 ",\"pause_ns\":%" PRIu64 ",\"maximum_pause_ns\":%" PRIu64
            ",\"committed_bytes\":%" PRIu64 ",\"minor_pause_ns\":%" PRIu64
            ",\"full_pause_ns\":%" PRIu64
            ",\"pause_bucket_upper_ns\":[10000,50000,100000,500000,1000000,5000000,10000000,null]"
            ",\"pause_buckets\":[%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 ",%" PRIu64
            ",%" PRIu64 ",%" PRIu64 ",%" PRIu64 "]}\n",
            result.minor_collections, result.full_collections, result.promotion_fallbacks,
            result.allocated_bytes, result.nursery_allocated_bytes, result.promoted_bytes, copied,
            result.dirty_cards, result.old_reference_slots, result.root_slots,
            result.traced_objects, result.pause_ns, result.maximum_pause_ns,
            result.heap_committed_bytes, minor_pause, full_pause, buckets[0], buckets[1],
            buckets[2], buckets[3], buckets[4], buckets[5], buckets[6], buckets[7]);
}
