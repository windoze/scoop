/* Reserve real to-space before publishing any forwarding or changing roots. */
#include <stdlib.h>
#include <string.h>

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

typedef struct MovingPlan {
    PlannedMove *moves;
    size_t count;
    size_t capacity;
    bool reuse_runs;
    bool allow_growth;
    bool reserved;
} MovingPlan;

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

static void rollback_reservations(void) {
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (block->state == SCOOP_BLOCK_EVACUATION_TARGET) {
            scoop_heap_release_block(block);
        } else if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE) {
            block->state =
                scoop_heap_block_has_pins(block) ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
            free(block->forwarding);
            block->forwarding = NULL;
        }
    }
    for (ScoopGcRegion *region = scoop_gc_heap_state.regions; region != NULL;
         region = region->next) {
        region->evacuation_source = false;
        region->evacuation_destination = false;
    }
    evacuation_block = NULL;
    evacuation_cursor = evacuation_limit = NULL;
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
    if (plan->count == plan->capacity) {
        size_t next = plan->capacity == 0 ? 256 : plan->capacity * 2;
        if (next < plan->capacity || next > SIZE_MAX / sizeof *plan->moves) {
            heap_fatal("evacuation plan size overflow");
        }
        PlannedMove *grown = realloc(plan->moves, next * sizeof *grown);
        if (grown == NULL) {
            heap_fatal("out of memory planning evacuation");
        }
        plan->moves = grown;
        plan->capacity = next;
    }
    size_t size = block->kind == SCOOP_BLOCK_KIND_LARGE
                      ? block->exact_size
                      : (size_t)block->size_units[word] * sizeof(uint64_t);
    const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)object)->td;
    ScoopGcBlockMeta *target;
    void *destination = reserve_target(size, (size_t)td->instance_shape.instance_alignment, &target,
                                       plan->reuse_runs, plan->allow_growth);
    if (destination == NULL) {
        plan->reserved = false;
        return;
    }
    plan->moves[plan->count++] = (PlannedMove){object, destination, size, word, block, target};
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
        for (size_t index = 0; index < plan.count; index++) {
            ScoopGcRegion *region = plan.moves[index].target_block->region;
            if (!region->evacuation_destination) {
                region->evacuation_destination = true;
                target_regions++;
                occupied_destinations += region->collection_live_bytes == 0;
            }
        }
        plan.reserved &= occupied_destinations < source_regions;
    }
    if (!plan.reserved) {
        free(plan.moves);
        rollback_reservations();
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
    for (size_t index = 0; index < plan.count; index++) {
        PlannedMove *move = &plan.moves[index];
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
    scoop_gc_heap_state.metrics.copy_ns += scoop_gc_monotonic_ns() - copy_started;
    free(plan.moves);
    return true;
}
