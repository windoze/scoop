/* Region-owned Immix blocks. All metadata lives outside managed mappings. */
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"

#undef collection_threshold
ScoopGcHeapState scoop_gc_heap_state = {
    .lock = PTHREAD_MUTEX_INITIALIZER,
    .collection_threshold = GC_INITIAL_THRESHOLD,
    .nursery_capacity = GC_NURSERY_MIN_CAPACITY,
    .nursery_limit = GC_NURSERY_DEFAULT_MAX_CAPACITY,
    .nursery_floor = GC_NURSERY_MIN_CAPACITY,
    .old_slots_level = 2,
    .incremental_enabled = true,
};
#define collection_threshold (scoop_gc_heap_state.collection_threshold)

_Noreturn void heap_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

void lock_heap(void) {
    if (pthread_mutex_lock(&heap_lock) != 0) {
        heap_fatal("failed to lock the heap");
    }
}

void unlock_heap(void) {
    if (pthread_mutex_unlock(&heap_lock) != 0) {
        heap_fatal("failed to unlock the heap");
    }
}

void scoop_gc_heap_lock(void) { lock_heap(); }
void scoop_gc_heap_unlock(void) { unlock_heap(); }

size_t scoop_heap_block_bytes(const ScoopGcBlockMeta *block) {
    return block->region->large ? block->region->size : GC_BLOCK_SIZE;
}

void free_run_nodes(void) {
    while (free_runs != NULL) {
        ScoopGcFreeRun *next = free_runs->next;
        free(free_runs);
        free_runs = next;
    }
}

void scoop_gc_heap_init(void) {
    if (scoop_gc_heap_state.ready) {
        heap_fatal("heap initialized more than once");
    }
    const char *survivor = getenv("SCOOP_GC_SURVIVOR");
    if (survivor != NULL) {
        if (strcmp(survivor, "0") != 0 && strcmp(survivor, "1") != 0) {
            heap_fatal("SCOOP_GC_SURVIVOR must be 0 or 1");
        }
        scoop_gc_heap_state.survivor_enabled = strcmp(survivor, "1") == 0;
    }
    const char *promotion = getenv("SCOOP_GC_PROMOTE_IN_PLACE");
    if (promotion != NULL) {
        if (strcmp(promotion, "0") != 0 && strcmp(promotion, "1") != 0) {
            heap_fatal("SCOOP_GC_PROMOTE_IN_PLACE must be 0 or 1");
        }
        scoop_gc_heap_state.promote_in_place = strcmp(promotion, "1") == 0;
    }
    const char *incremental = getenv("SCOOP_GC_INCREMENTAL");
    if (incremental != NULL) {
        if (strcmp(incremental, "0") != 0 && strcmp(incremental, "1") != 0 &&
            strcmp(incremental, "2") != 0) {
            heap_fatal("SCOOP_GC_INCREMENTAL must be 0, 1 or 2");
        }
        scoop_gc_heap_state.incremental_enabled = strcmp(incremental, "0") != 0;
        scoop_gc_heap_state.incremental_minor_enabled = strcmp(incremental, "2") == 0;
    }
    const char *slots = getenv("SCOOP_GC_OLD_SLOTS");
    if (slots != NULL) {
        if (strcmp(slots, "0") != 0 && strcmp(slots, "1") != 0 && strcmp(slots, "2") != 0 &&
            strcmp(slots, "3") != 0) {
            heap_fatal("SCOOP_GC_OLD_SLOTS must be 0, 1, 2 or 3");
        }
        scoop_gc_heap_state.old_slots_level = (uint8_t)(slots[0] - '0');
    }
    const char *nursery = getenv("SCOOP_GC_NURSERY_MAX_MIB");
    if (nursery != NULL) {
        char *end;
        errno = 0;
        unsigned long long mib = strtoull(nursery, &end, 10);
        if (nursery[0] < '0' || nursery[0] > '9' || *end != '\0' || errno == ERANGE ||
            mib == 0 || mib > SIZE_MAX / GC_NURSERY_MIN_CAPACITY) {
            heap_fatal("SCOOP_GC_NURSERY_MAX_MIB must be a positive representable MiB count");
        }
        scoop_gc_heap_state.nursery_limit = (size_t)mib * GC_NURSERY_MIN_CAPACITY;
    }
#if SCOOP_ENABLE_GC_STRESS
    const char *stress = getenv("SCOOP_GC_STRESS_MOVE");
    stress_move = stress != NULL && strcmp(stress, "1") == 0;
    const char *minor = getenv("SCOOP_GC_STRESS_MINOR");
    scoop_gc_heap_state.stress_minor = minor != NULL && strcmp(minor, "1") == 0;
#endif
    const char *full = getenv("SCOOP_GC_FULL_ONLY");
    scoop_gc_heap_state.full_only = full != NULL && strcmp(full, "1") == 0;
    const char *metrics = getenv("SCOOP_GC_STATS");
    scoop_gc_heap_state.print_metrics = metrics != NULL && strcmp(metrics, "1") == 0;
    scoop_gc_heap_state.ready = true;
}

void require_heap(void) {
    if (!scoop_gc_heap_state.ready) {
        heap_fatal("heap used before initialization");
    }
}

#if SCOOP_ENABLE_GC_STRESS
bool scoop_gc_stress_move_enabled(void) {
    require_heap();
    return stress_move;
}
#endif

ScoopGcBlockMeta *scoop_heap_first_block(void) {
    ScoopGcRegion *region = scoop_gc_heap_state.regions;
    while (region != NULL && region->next_block == 0) {
        region = region->next;
    }
    return region == NULL ? NULL : region->blocks;
}

ScoopGcBlockMeta *scoop_heap_next_block(const ScoopGcBlockMeta *block) {
    ScoopGcRegion *region = block->region;
    if ((size_t)block->index + 1 < region->next_block) {
        return &region->blocks[block->index + 1];
    }
    region = region->next;
    while (region != NULL && region->next_block == 0) {
        region = region->next;
    }
    return region == NULL ? NULL : region->blocks;
}

ScoopGcBlockMeta *pointer_block(const void *pointer) {
    uintptr_t address = (uintptr_t)pointer;
    ScoopGcRegion *region = scoop_heap_region_for_address(address);
    if (region == NULL) {
        return NULL;
    }
    size_t index = region->large ? 0 : (address - region->base) / GC_BLOCK_SIZE;
    return &region->blocks[index];
}

bool bit_test(const uint64_t *bits, size_t index) {
    return ((bits[index / 64] >> (index % 64)) & UINT64_C(1)) != 0;
}

void bit_set(uint64_t *bits, size_t index) { bits[index / 64] |= UINT64_C(1) << (index % 64); }

void bit_clear(uint64_t *bits, size_t index) { bits[index / 64] &= ~(UINT64_C(1) << (index % 64)); }

static void ensure_small_metadata(ScoopGcBlockMeta *block) {
    if (block->line_live == NULL) {
#if SCOOP_GC_TRACK_RELOCATION
        block->scanned = calloc(GC_BITMAP_WORDS, sizeof(uint64_t));
        if (block->scanned == NULL) {
            heap_fatal("out of memory allocating relocation coverage bitmap");
        }
#endif
        block->line_live = calloc(GC_LINE_BITMAP_WORDS, sizeof(uint64_t));
        if (block->line_live == NULL) {
            heap_fatal("out of memory allocating small-block side metadata");
        }
    }
}

static void reset_small_metadata(ScoopGcBlockMeta *block) {
    ensure_small_metadata(block);
    memset(block->starts, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    if (block->pins != NULL) {
        memset(block->pins, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
    }
#if SCOOP_GC_TRACK_RELOCATION
    memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
#endif
    memset(block->line_live, 0, GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
}

static ScoopGcBlockMeta *take_small_block(bool allow_growth) {
    ScoopGcBlockMeta **link = &scoop_gc_heap_state.free_blocks;
    while (*link != NULL && (*link)->region->evacuation_source) {
        link = &(*link)->next_free;
    }
    ScoopGcBlockMeta *block = *link;
    if (block != NULL) {
        *link = block->next_free;
        block->next_free = NULL;
        return block;
    }
    ScoopGcRegion **available = &scoop_gc_heap_state.available_regions;
    while (*available != NULL && (*available)->evacuation_source) {
        available = &(*available)->next_available;
    }
    ScoopGcRegion *region = *available;
    if (region == NULL) {
        if (!allow_growth) {
            return NULL;
        }
        region = scoop_heap_region_create(GC_REGION_SIZE, false);
        if (region == NULL) {
            return NULL;
        }
        available = &scoop_gc_heap_state.available_regions;
    }
    block = &region->blocks[region->next_block++];
    if (region->next_block == GC_REGION_BLOCKS) {
        *available = region->next_available;
        region->next_available = NULL;
    }
    return block;
}

static void record_evacuation_target(ScoopGcBlockMeta *block) {
    block->next_collection = scoop_gc_heap_state.evacuation_targets;
    scoop_gc_heap_state.evacuation_targets = block;
}

ScoopGcBlockMeta *activate_small_block(ScoopGcBlockState state, bool allow_growth) {
    ScoopGcBlockMeta *block = take_small_block(allow_growth);
    if (block == NULL) {
        return NULL;
    }
    if (block->head->state != SCOOP_BLOCK_NEVER_USED && block->head->state != SCOOP_BLOCK_FREE) {
        heap_fatal("region allocator selected an active block");
    }
    reset_small_metadata(block);
    block->head->state = state;
    block->head->kind = SCOOP_BLOCK_KIND_SMALL;
    block->head->generation = SCOOP_GC_OLD;
    block->survivor = false;
    block->retained_young = false;
    block->exact_size = 0;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    block->mark_index = SIZE_MAX;
    block->discard_pending = true;
    committed_bytes += GC_BLOCK_SIZE;
    active_block_heads++;
    if (state == SCOOP_BLOCK_EVACUATION_TARGET) {
        record_evacuation_target(block);
    }
    return block;
}

ScoopGcBlockMeta *activate_large_block(size_t exact_size, ScoopGcBlockState state) {
    ScoopGcRegion *region =
        scoop_heap_region_create(scoop_heap_large_mapping_size(exact_size), true);
    if (region == NULL) {
        return NULL;
    }
    ScoopGcBlockMeta *block = region->blocks;
    block->head->state = state;
    block->head->kind = SCOOP_BLOCK_KIND_LARGE;
    block->head->generation = SCOOP_GC_OLD;
    block->survivor = false;
    block->retained_young = false;
    block->exact_size = exact_size;
    block->mark_index = SIZE_MAX;
    region->next_block = 1;
    committed_bytes += region->size;
    active_block_heads++;
    if (state == SCOOP_BLOCK_EVACUATION_TARGET) {
        record_evacuation_target(block);
    }
    return block;
}
