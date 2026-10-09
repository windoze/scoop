/* Post-verification object retirement, ordinary reuse and stress quarantine. */
#include <stdatomic.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "../thread.h"

void scoop_gc_heap_finish_collection_locked(uint64_t object_count, bool minor) {
    if (!collection_active) {
        heap_fatal("heap collection finished without begin");
    }
    ScoopAllocationTotals allocations = scoop_thread_allocation_totals_locked();
    if (minor) {
        uint64_t allocated =
            allocations.objects - scoop_gc_heap_state.allocation_objects_at_collection;
        uint64_t young =
            allocations.nursery_objects - scoop_gc_heap_state.nursery_objects_at_collection;
        object_count += scoop_gc_heap_state.collected_live_objects + allocated - young;
    } else {
        free_run_nodes();
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || (minor && block->generation == SCOOP_GC_OLD &&
                                    block->state != SCOOP_BLOCK_EVACUATION_TARGET)) {
            continue;
        }
        if (block->generation == SCOOP_GC_YOUNG) {
            scoop_gc_heap_state.metrics.promoted_bytes += block->live_bytes;
        }
        block->generation = SCOOP_GC_OLD;
        scoop_heap_finish_block(block, stress_move);
    }
    evacuation_block = NULL;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    scoop_heap_reclaim_regions(!minor && !stress_move);
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        memset(region->cards, 0, region->size >> GC_CARD_SHIFT);
        region->evacuation_source = false;
        region->evacuation_destination = false;
    }
    scoop_gc_heap_state.nursery_bytes = 0;
    scoop_gc_heap_state.collected_live_objects = object_count;
    scoop_gc_heap_state.allocation_objects_at_collection = allocations.objects;
    scoop_gc_heap_state.nursery_objects_at_collection = allocations.nursery_objects;
    atomic_store_explicit(&last_moved_objects, moved_objects, memory_order_release);
    if (!minor) {
        size_t next = committed_bytes > SIZE_MAX / 2 ? SIZE_MAX : committed_bytes * 2;
        collection_threshold = next > GC_INITIAL_THRESHOLD ? next : GC_INITIAL_THRESHOLD;
    }
    collection_active = false;
}
