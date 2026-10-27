/* Reserve all to-space before copying objects or changing any live entry/root. */
#include <string.h>

#include "../gc_binding/object.h"
#include "allocation_metadata.h"
#include "gc_internal.h"
#include "heap_internal.h"

/* A reserved target is not an object until copying replaces this header. */
typedef struct PendingMove {
    struct PendingMove *next;
    void **entry;
} PendingMove;

_Static_assert(sizeof(PendingMove) == sizeof(ScoopObjectHeader) &&
                   _Alignof(PendingMove) <= _Alignof(ScoopObjectHeader),
               "pending move must fit in the unpublished target header");

typedef struct MovingPlan {
    PendingMove *first;
    PendingMove *last;
    ScoopGcBlockMeta *survivor_block;
    char *survivor_cursor;
    char *survivor_limit;
    size_t survivor_capacity_limit;
    ScoopGcSurvivors survivors;
    size_t target_regions;
    size_t occupied_destinations;
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

static void reserve_move(void **entry, void *context) {
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
                      : scoop_heap_small_object_size(object);
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
    PendingMove *move = destination;
    *move = (PendingMove){.entry = entry};
    if (plan->last == NULL) {
        plan->first = move;
    } else {
        plan->last->next = move;
    }
    plan->last = move;
    if (plan->reuse_runs && !target->region->evacuation_destination) {
        target->region->evacuation_destination = true;
        plan->target_regions++;
        plan->occupied_destinations += target->region->collection_live_bytes == 0;
    }
}

static void copy_reserved_objects(PendingMove *move) {
    ScoopGcBlockMeta *target_block = NULL;
    uintptr_t target_base = 0;
    while (move != NULL) {
        /* Copying destroys the temporary header, so take both fields first. */
        PendingMove *next = move->next;
        void **entry = move->entry;
        ScoopObjectHeader *source = *entry;
        size_t size = __atomic_load_n(&source->gc_word, __ATOMIC_RELAXED) & GC_SMALL_SIZE_MASK;
        ScoopGcBlockMeta *block;
        if (size == 0) {
            block = pointer_block(move);
            size = block->exact_size;
        } else {
            if (target_block == NULL || (uintptr_t)move - target_base >= GC_BLOCK_SIZE) {
                target_block = pointer_block(move);
                target_base = (uintptr_t)block_base(target_block);
            }
            block = target_block;
        }
        memcpy(move, source, size);
        scoop_gc_heap_state.copied_bytes += size;
        if (size <= GC_REGULAR_MAX) {
            scoop_heap_record_small_metadata(block, move, size, true, true);
        } else {
            publish_large_object(block, true);
        }
        __atomic_store_n(&source->gc_word, (uintptr_t)move | GC_FORWARDED_BIT, __ATOMIC_RELAXED);
        *entry = move;
        moved_objects++;
        move = next;
    }
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
    scoop_gc_mark_visit_live(reserve_move, &plan);
    if (ordinary_full) {
        plan.reserved &= plan.occupied_destinations < source_regions;
    }
    if (!plan.reserved) {
        rollback_reservations(minor);
        if (stress_move) {
            heap_fatal("stress heap exhausted during evacuation reservation");
        }
        return false;
    }
    if (ordinary_full) {
        scoop_gc_heap_state.metrics.last_full_source_regions = source_regions;
        scoop_gc_heap_state.metrics.last_full_target_regions = plan.target_regions;
    }
    uint64_t copy_started = scoop_gc_monotonic_ns();
    copy_reserved_objects(plan.first);
    scoop_gc_heap_state.metrics.copy_ns += scoop_gc_monotonic_ns() - copy_started;
    *survivors = plan.survivors;
    return true;
}
