/* Post-verification object retirement, ordinary reuse and stress quarantine. */
#include <stdatomic.h>

#include "gc_internal.h"
#include "heap_internal.h"

static void finish_block(ScoopGcBlockMeta *block, bool minor) {
    if (block->generation == SCOOP_GC_YOUNG) {
        scoop_gc_heap_state.metrics.promoted_bytes += block->live_bytes;
        if (minor && block->kind == SCOOP_BLOCK_KIND_SMALL &&
            (block->live_bytes == 0 || block->state == SCOOP_BLOCK_EVACUATION_SOURCE)) {
            /* Minor sources are unpinned and fully updated; activation resets metadata. */
            scoop_heap_release_block(block);
            return;
        }
    }
    block->generation = SCOOP_GC_OLD;
    scoop_heap_finish_block(block, stress_move);
}

void scoop_gc_heap_finish_collection_locked(uint64_t object_count, bool minor) {
    if (!collection_active) {
        heap_fatal("heap collection finished without begin");
    }
    ScoopGcAllocationTotals allocations = scoop_gc_allocation_totals_locked();
    if (minor) {
        uint64_t allocated =
            allocations.objects - scoop_gc_heap_state.allocation_objects_at_collection;
        uint64_t young =
            allocations.nursery_objects - scoop_gc_heap_state.nursery_objects_at_collection;
        object_count += scoop_gc_heap_state.collected_live_objects + allocated - young;
    } else {
        free_run_nodes();
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_collection_block(minor); block != NULL;) {
        /* Retirement reuses the link for the free list. */
        ScoopGcBlockMeta *next = scoop_heap_next_collection_block(block, minor);
        if (active_head(block)) {
            finish_block(block, minor);
        }
        block = next;
    }
    if (minor) {
        while (scoop_gc_heap_state.evacuation_targets != NULL) {
            ScoopGcBlockMeta *block = scoop_gc_heap_state.evacuation_targets;
            scoop_gc_heap_state.evacuation_targets = block->next_collection;
            finish_block(block, true);
        }
    }
    scoop_gc_heap_state.young_blocks = NULL;
    scoop_gc_heap_state.evacuation_targets = NULL;
    evacuation_block = NULL;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    scoop_heap_clear_dirty_card_groups();
    if (!minor) {
        uint64_t vm_started = scoop_gc_monotonic_ns();
        scoop_heap_reclaim_regions(!stress_move);
        scoop_gc_heap_state.metrics.vm_return_ns += scoop_gc_monotonic_ns() - vm_started;
        for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
             region = region->next) {
            region->evacuation_source = false;
            region->evacuation_destination = false;
        }
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
