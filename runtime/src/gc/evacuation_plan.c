/* Reserve real to-space before publishing any forwarding or changing roots. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "gc_internal.h"
#include "heap_internal.h"

typedef struct PlannedMove {
    void *source;
    void *destination;
    size_t size;
    size_t word;
    ScoopGcBlockMeta *source_block;
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
    bool reuse_runs;
    bool allow_growth;
    bool reserved;
} MovingPlan;

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
        if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE) {
            block->state =
                scoop_heap_block_has_pins(block) ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
            free(block->forwarding);
            block->forwarding = NULL;
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

static void append_move(void *object, void *context) {
    MovingPlan *plan = context;
    if (!plan->reserved) {
        return;
    }
    ScoopGcBlockMeta *block = pointer_block(object);
    size_t word = ((uintptr_t)object - (uintptr_t)block_base(block)) / sizeof(uint64_t);
    if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE || object_pinned(block, word)) {
        return;
    }
    size_t size = block->kind == SCOOP_BLOCK_KIND_LARGE
                      ? block->exact_size
                      : (size_t)block->size_units[word] * sizeof(uint64_t);
    ScoopGcBlockMeta *target;
    void *destination = reserve_target(size, scoop_gc_object_alignment(object), &target,
                                       plan->reuse_runs, plan->allow_growth);
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
    plan->last->moves[plan->last->used++] =
        (PlannedMove){object, destination, size, word, block, target};
}

bool scoop_gc_heap_plan_moving_locked(bool minor) {
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    size_t source_regions = scoop_heap_select_evacuation_sources(minor);
    bool ordinary_full = !minor && !stress_move;
    if (ordinary_full) {
        scoop_gc_heap_state.metrics.last_full_source_regions = 0;
        scoop_gc_heap_state.metrics.last_full_target_regions = 0;
    }
    if (ordinary_full && source_regions == 0) {
        return true;
    }
    if (ordinary_full && source_regions == 1 && !single_source_may_fit()) {
        rollback_reservations(minor);
        return false;
    }
    if (ordinary_full) {
        scoop_heap_prepare_evacuation_targets();
    }
    MovingPlan plan = {.reuse_runs = ordinary_full,
                       .allow_growth = !ordinary_full || source_regions > 1,
                       .reserved = true};
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
            memcpy(move->destination, move->source, move->size);
            scoop_gc_heap_state.copied_bytes += move->size;
            ScoopGcBlockMeta *source = move->source_block;
            if (move->size <= GC_REGULAR_MAX) {
                record_small_object(move->target_block, move->destination, move->size, true);
                source->forwarding[move->word] = move->destination;
            } else {
                publish_large_object(move->target_block, true);
                source->large_forwarding = move->destination;
            }
            moved_objects++;
        }
    }
    scoop_gc_heap_state.metrics.copy_ns += scoop_gc_monotonic_ns() - copy_started;
    free_plan(plan.first);
    return true;
}
