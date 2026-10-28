/* Post-verification object retirement, ordinary reuse and stress quarantine. */
#include <stdatomic.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "old_slots.h"

static size_t nursery_target(size_t bytes) {
    if (bytes < GC_NURSERY_MIN_CAPACITY) {
        return GC_NURSERY_MIN_CAPACITY;
    }
    if (bytes >= scoop_gc_heap_state.nursery_limit) {
        return scoop_gc_heap_state.nursery_limit;
    }
    return (bytes + GC_BLOCK_SIZE - 1) & ~(GC_BLOCK_SIZE - 1);
}

static void adjust_nursery(size_t used, uint64_t live, bool minor) {
    if (!scoop_heap_adaptive_nursery_enabled()) {
        return;
    }
    ScoopGcHeapState *heap = &scoop_gc_heap_state;
    if (!minor) {
        heap->nursery_floor = nursery_target(committed_bytes / 2);
    }
    size_t capacity = heap->nursery_capacity;
    if (capacity < heap->nursery_floor) {
        capacity = heap->nursery_floor;
    }
    if (used >= capacity - capacity / 4 && live >= used / 4) {
        capacity = capacity > heap->nursery_limit / 2 ? heap->nursery_limit : capacity * 2;
        heap->nursery_low_survival_collections = 0;
    } else if (used != 0 || !minor) {
        if (live < used / 16 || used == 0) {
            if (++heap->nursery_low_survival_collections == 4) {
                capacity = nursery_target(capacity / 2);
                if (capacity < heap->nursery_floor) {
                    capacity = heap->nursery_floor;
                }
                heap->nursery_low_survival_collections = 0;
            }
        } else {
            heap->nursery_low_survival_collections = 0;
        }
    }
    heap->nursery_capacity = capacity;
}

static void finish_block(ScoopGcBlockMeta *block, bool minor) {
    if (block->head->generation == SCOOP_GC_YOUNG) {
        if (minor && block->head->kind == SCOOP_BLOCK_KIND_SMALL &&
            (block->live_bytes == 0 || block->head->state == SCOOP_BLOCK_EVACUATION_SOURCE)) {
            /* Minor sources are unpinned and fully updated; activation resets metadata. */
            scoop_heap_release_block(block);
            return;
        }
    }
    bool promoted_slots = minor && block->head->generation == SCOOP_GC_YOUNG &&
                          block->head->kind == SCOOP_BLOCK_KIND_SMALL && block->slot_size != 0;
    bool retained = minor && block->head->generation == SCOOP_GC_YOUNG && block->retained_young;
    if (!retained) {
        block->head->generation = SCOOP_GC_OLD;
    }
    block->survivor = retained;
    block->retained_young = false;
    scoop_heap_finish_block(block, stress_move);
    if ((!minor || promoted_slots) && active_head(block) && block->head->kind == SCOOP_BLOCK_KIND_SMALL &&
        block->slot_size != 0) {
        scoop_heap_record_old_slots(block);
    }
}

void scoop_gc_heap_finish_collection_locked(uint64_t object_count, bool minor,
                                             const ScoopGcSurvivors *survivors) {
    if (!collection_active) {
        heap_fatal("heap collection finished without begin");
    }
    ScoopGcAllocationTotals allocations = scoop_gc_allocation_totals_locked();
    size_t nursery_used = scoop_gc_heap_state.nursery_bytes;
    uint64_t young_live = 0;
    size_t full_live = 0;
    if (minor) {
        uint64_t allocated =
            allocations.objects - scoop_gc_heap_state.allocation_objects_at_collection;
        uint64_t young =
            allocations.nursery_objects - scoop_gc_heap_state.nursery_objects_at_collection;
        object_count += scoop_gc_heap_state.collected_live_objects -
                        scoop_gc_heap_state.survivor_objects + allocated - young;
    } else {
        free_run_nodes();
        scoop_heap_clear_old_slots();
    }
    ScoopGcBlockMeta *young_blocks = NULL;
    for (ScoopGcBlockMeta *block = scoop_heap_first_collection_block(minor); block != NULL;) {
        /* Retirement reuses the link for the free list. */
        ScoopGcBlockMeta *next = scoop_heap_next_collection_block(block, minor);
        if (active_head(block)) {
            if (!minor && block->head->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
                full_live += block->live_bytes;
            }
            if (block->head->generation == SCOOP_GC_YOUNG) {
                young_live += block->live_bytes;
            }
            finish_block(block, minor);
            if (minor && block->head->generation == SCOOP_GC_YOUNG) {
                block->next_collection = young_blocks;
                young_blocks = block;
            }
        }
        block = next;
    }
    if (minor) {
        while (scoop_gc_heap_state.evacuation_targets != NULL) {
            ScoopGcBlockMeta *block = scoop_gc_heap_state.evacuation_targets;
            scoop_gc_heap_state.evacuation_targets = block->next_collection;
            finish_block(block, true);
            if (block->head->generation == SCOOP_GC_YOUNG) {
                block->next_collection = young_blocks;
                young_blocks = block;
            }
        }
    }
    scoop_gc_heap_state.young_blocks = young_blocks;
    scoop_gc_heap_state.evacuation_targets = NULL;
    if (minor) {
        scoop_heap_finish_old_slot_reservations();
    }
    scoop_heap_clear_dirty_card_groups(survivors->objects != 0);
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
    scoop_gc_heap_state.metrics.promoted_bytes += young_live - survivors->live_bytes;
    if (minor) {
        scoop_heap_note_old_bytes(young_live - survivors->live_bytes);
    }
    adjust_nursery(nursery_used, young_live, minor);
    scoop_gc_heap_state.nursery_bytes = survivors->capacity;
    scoop_gc_heap_state.survivor_objects = survivors->objects;
    scoop_gc_heap_state.collected_live_objects = object_count;
    scoop_gc_heap_state.allocation_objects_at_collection = allocations.objects;
    scoop_gc_heap_state.nursery_objects_at_collection = allocations.nursery_objects;
    atomic_store_explicit(&last_moved_objects, moved_objects, memory_order_release);
    if (!minor) {
        size_t basis = committed_bytes;
        if (scoop_heap_uses_old_slots()) {
            scoop_gc_heap_state.old_pressure_bytes = full_live;
            basis = full_live;
        }
        size_t next = basis > SIZE_MAX / 2 ? SIZE_MAX : basis * 2;
        collection_threshold = next > GC_INITIAL_THRESHOLD ? next : GC_INITIAL_THRESHOLD;
    }
    collection_active = false;
}
