/* Ordinary full collections compact sparse regions; stress/minor select blocks. */
#include <stdlib.h>

#include "heap_internal.h"

bool scoop_heap_block_has_pins(const ScoopGcBlockMeta *block) {
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        return block->large_pinned;
    }
    for (size_t index = 0; index < GC_BITMAP_WORDS; index++) {
        if (block->pins[index] != 0) {
            return true;
        }
    }
    return false;
}

static void select_block(ScoopGcBlockMeta *source) {
    source->state = SCOOP_BLOCK_EVACUATION_SOURCE;
    if (source->kind == SCOOP_BLOCK_KIND_SMALL) {
        source->forwarding = calloc(GC_WORDS_PER_BLOCK, sizeof(void *));
        if (source->forwarding == NULL) {
            heap_fatal("out of memory allocating forwarding metadata");
        }
    }
}

static size_t select_regions(void) {
    ScoopGcRegion *destination = NULL;
    size_t destination_live = 0, count = 0;
    size_t pin_blocked = 0;
    bool destination_pin_blocked = false;
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        if (region->large) {
            continue;
        }
        size_t live = 0;
        bool pinned = false;
        for (size_t index = 0; index < region->next_block; index++) {
            ScoopGcBlockMeta *block = &region->blocks[index];
            if (active_head(block)) {
                live += block->live_bytes;
                pinned |= scoop_heap_block_has_pins(block);
            }
        }
        bool blocked = live != 0 && live <= GC_REGION_SIZE / 4 && pinned;
        pin_blocked += blocked;
        if (live > destination_live) {
            destination_live = live;
            destination = region;
            destination_pin_blocked = blocked;
        }
        region->evacuation_source = live != 0 && live <= GC_REGION_SIZE / 4 && !pinned;
        region->collection_live_bytes = live;
        count += region->evacuation_source;
    }
    if (destination != NULL && destination->evacuation_source) {
        destination->evacuation_source = false;
        count--;
    }
    scoop_gc_heap_state.metrics.last_full_pin_blocked_regions =
        pin_blocked - destination_pin_blocked;
    return count;
}

size_t scoop_heap_select_evacuation_sources(bool minor) {
    size_t regions = !minor && !stress_move ? select_regions() : 0;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || block->movable_live_bytes == 0) {
            continue;
        }
        bool selected =
            minor ? block->generation == SCOOP_GC_YOUNG && !scoop_heap_block_has_pins(block)
                  : stress_move || block->region->evacuation_source;
        if (selected) {
            select_block(block);
        }
    }
    return regions;
}
