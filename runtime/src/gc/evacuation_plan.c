/* Reserve real to-space before publishing any forwarding or changing roots. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "allocation_metadata.h"
#include "gc_internal.h"
#include "heap_internal.h"

typedef struct PlannedMove {
    void **entry;
    void *destination;
    size_t size;
    ScoopGcBlockMeta *target_block;
} PlannedMove;

#define GC_MOVE_CHUNK 256

typedef struct PlannedMoveChunk {
    struct PlannedMoveChunk *next;
    size_t used;
    PlannedMove moves[GC_MOVE_CHUNK];
} PlannedMoveChunk;

typedef struct MovingPlan {
    PlannedMoveChunk *first;
    PlannedMoveChunk *last;
    ScoopGcBlockMeta *survivor_block;
    char *survivor_cursor;
    char *survivor_limit;
    size_t survivor_capacity_limit;
    ScoopGcSurvivors survivors;
    bool reuse_runs;
    bool allow_growth;
    bool reserved;
} MovingPlan;

static void *reserve_survivor(MovingPlan *plan, size_t size, size_t alignment,
                              ScoopGcBlockMeta **target) {
    void *object = scoop_heap_bump(&plan->survivor_cursor, plan->survivor_limit, size, alignment);
    if (object == NULL) {
        if (plan->survivors.capacity + GC_BLOCK_SIZE > plan->survivor_capacity_limit) {
            return NULL;
        }
        ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET, true);
        if (block == NULL) {
            return NULL;
        }
        block->generation = SCOOP_GC_YOUNG;
        block->survivor = true;
        block->retained_young = true;
        plan->survivor_block = block;
        plan->survivor_cursor = (char *)block_base(block) + GC_LINE_SIZE;
        plan->survivor_limit = (char *)block_base(block) + GC_BLOCK_SIZE;
        plan->survivors.capacity += GC_BLOCK_SIZE;
        object = scoop_heap_bump(&plan->survivor_cursor, plan->survivor_limit, size, alignment);
    }
    *target = plan->survivor_block;
    plan->survivors.live_bytes += size;
    plan->survivors.objects++;
    return object;
}

static void free_plan(PlannedMoveChunk *chunk) {
    while (chunk != NULL) {
        PlannedMoveChunk *next = chunk->next;
        free(chunk);
        chunk = next;
    }
}

static void *reserve_target(size_t size, size_t alignment, ScoopGcBlockMeta **block,
                            bool reuse_runs, bool allow_growth) {
    if (size > GC_REGULAR_MAX) {
        *block = activate_large_block(size, SCOOP_BLOCK_EVACUATION_TARGET);
        return *block == NULL ? NULL : (char *)block_base(*block) + GC_LINE_SIZE;
    }
    void *object = scoop_heap_bump(&evacuation_cursor, evacuation_limit, size, alignment);
    if (object == NULL && reuse_runs) {
        ScoopGcBlockMeta *reused =
            scoop_heap_take_free_run(size, &evacuation_cursor, &evacuation_limit);
        if (reused != NULL) {
            evacuation_block = reused;
            object = scoop_heap_bump(&evacuation_cursor, evacuation_limit, size, alignment);
        }
    }
    if (object == NULL) {
        evacuation_block = activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET, allow_growth);
        if (evacuation_block == NULL) {
            return NULL;
        }
        evacuation_cursor = (char *)block_base(evacuation_block) + GC_LINE_SIZE;
        evacuation_limit = (char *)block_base(evacuation_block) + GC_BLOCK_SIZE;
        object = scoop_heap_bump(&evacuation_cursor, evacuation_limit, size, alignment);
    }
    *block = evacuation_block;
    return object;
}

static void rollback_reservations(bool minor) {
    while (scoop_gc_heap_state.evacuation_targets != NULL) {
        ScoopGcBlockMeta *block = scoop_gc_heap_state.evacuation_targets;
        scoop_gc_heap_state.evacuation_targets = block->next_collection;
        scoop_heap_release_block(block);
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_collection_block(minor); block != NULL;
         block = scoop_heap_next_collection_block(block, minor)) {
        block->retained_young = false;
        if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE) {
            block->state =
                scoop_heap_block_has_pins(block) ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
        }
    }
    if (!minor) {
        for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
             region = region->next) {
            region->evacuation_source = false;
            region->evacuation_destination = false;
        }
    }
    evacuation_block = NULL;
    evacuation_cursor = evacuation_limit = NULL;
}

static bool single_source_may_fit(void) {
    size_t required = 0;
    for (const ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        if (region->evacuation_source) {
            required = region->collection_live_bytes;
            break;
        }
    }
    for (const ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        if (region->large || region->evacuation_source || region->collection_live_bytes == 0) {
            continue;
        }
        /* A new occupied region cannot reduce one source. This upper bound
         * ignores fragmentation and alignment; success still needs reservation. */
        size_t available = GC_REGION_BLOCKS * GC_REGULAR_MAX - region->collection_live_bytes;
        if (available >= required) {
            return true;
        }
        required -= available;
    }
    return false;
}

static void append_move(void **entry, void *context) {
    MovingPlan *plan = context;
    if (!plan->reserved) {
        return;
    }
    void *object = *entry;
    ScoopGcBlockMeta *block = pointer_block(object);
    size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
    if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE || object_pinned(block, word)) {
        plan->survivors.objects += block->retained_young;
        return;
    }
    size_t size = block->kind == SCOOP_BLOCK_KIND_LARGE
                      ? block->exact_size
                      : (size_t)block->size_units[word] * sizeof(uint64_t);
    ScoopGcBlockMeta *target;
    size_t alignment = scoop_gc_object_alignment(object);
    void *destination = NULL;
    if (plan->survivor_capacity_limit != 0 && !block->survivor &&
        block->kind == SCOOP_BLOCK_KIND_SMALL) {
        destination = reserve_survivor(plan, size, alignment, &target);
    }
    if (destination == NULL) {
        destination = reserve_target(size, alignment, &target, plan->reuse_runs, plan->allow_growth);
    }
    if (destination == NULL) {
        plan->reserved = false;
        return;
    }
    if (plan->last == NULL || plan->last->used == GC_MOVE_CHUNK) {
        PlannedMoveChunk *chunk = malloc(sizeof *chunk);
        if (chunk == NULL) {
            heap_fatal("out of memory planning evacuation");
        }
        chunk->next = NULL;
        chunk->used = 0;
        if (plan->last == NULL) {
            plan->first = chunk;
        } else {
            plan->last->next = chunk;
        }
        plan->last = chunk;
    }
    plan->last->moves[plan->last->used++] = (PlannedMove){entry, destination, size, target};
}

bool scoop_gc_heap_plan_moving_locked(bool minor, ScoopGcSurvivors *survivors) {
    *survivors = (ScoopGcSurvivors){0};
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    bool ordinary_full = !minor && !stress_move;
    MovingPlan plan = {.reuse_runs = ordinary_full, .reserved = true};
    if (minor && scoop_gc_heap_state.survivor_enabled && scoop_heap_adaptive_nursery_enabled()) {
        plan.survivor_capacity_limit = scoop_gc_heap_state.nursery_capacity / 4;
    }
    bool has_sources;
    size_t source_regions = scoop_heap_select_evacuation_sources(
        minor, plan.survivor_capacity_limit, &plan.survivors, &has_sources);
    plan.allow_growth = !ordinary_full || source_regions > 1;
    if (ordinary_full) {
        scoop_gc_heap_state.metrics.last_full_source_regions = 0;
        scoop_gc_heap_state.metrics.last_full_target_regions = 0;
    }
    if (!has_sources && plan.survivors.capacity == 0) {
        return true;
    }
    if (ordinary_full && source_regions == 1 && !single_source_may_fit()) {
        rollback_reservations(minor);
        return false;
    }
    if (ordinary_full) {
        scoop_heap_prepare_evacuation_targets();
    }
    scoop_gc_mark_visit_live(append_move, &plan);
    size_t target_regions = 0;
    if (ordinary_full) {
        size_t occupied_destinations = 0;
        for (PlannedMoveChunk *chunk = plan.first; chunk != NULL; chunk = chunk->next) {
            for (size_t index = 0; index < chunk->used; index++) {
                ScoopGcRegion *region = chunk->moves[index].target_block->region;
                if (!region->evacuation_destination) {
                    region->evacuation_destination = true;
                    target_regions++;
                    occupied_destinations += region->collection_live_bytes == 0;
                }
            }
        }
        plan.reserved &= occupied_destinations < source_regions;
    }
    if (!plan.reserved) {
        free_plan(plan.first);
        rollback_reservations(minor);
        if (stress_move) {
            heap_fatal("stress heap exhausted during evacuation reservation");
        }
        return false;
    }
    if (ordinary_full) {
        scoop_gc_heap_state.metrics.last_full_source_regions = source_regions;
        scoop_gc_heap_state.metrics.last_full_target_regions = target_regions;
    }
    uint64_t copy_started = scoop_gc_monotonic_ns();
    for (PlannedMoveChunk *chunk = plan.first; chunk != NULL; chunk = chunk->next) {
        for (size_t index = 0; index < chunk->used; index++) {
            PlannedMove *move = &chunk->moves[index];
            ScoopObjectHeader *source = *move->entry;
            memcpy(move->destination, source, move->size);
            scoop_gc_heap_state.copied_bytes += move->size;
            if (move->size <= GC_REGULAR_MAX) {
                /* The complete plan owns these ranges; mark/update workers are idle. */
                scoop_heap_record_small_metadata(move->target_block, move->destination, move->size,
                                                 true, true);
            } else {
                publish_large_object(move->target_block, true);
            }
            /* Preserve the original GC word in the current copy before reusing
             * the inaccessible source header for forwarding. */
            __atomic_store_n(&source->gc_word, (uintptr_t)move->destination | GC_FORWARDED_BIT,
                             __ATOMIC_RELAXED);
            /* Mark tasks have finished; update tasks will only see current copies. */
            *move->entry = move->destination;
            moved_objects++;
        }
    }
    scoop_gc_heap_state.metrics.copy_ns += scoop_gc_monotonic_ns() - copy_started;
    free_plan(plan.first);
    *survivors = plan.survivors;
    return true;
}
