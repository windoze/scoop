/* Reserve all to-space before copying objects or changing any live entry/root. */
#include <stdlib.h>
#include <string.h>

#include "../gc_binding/object.h"
#include "allocation_metadata.h"
#include "gc_internal.h"
#include "heap_internal.h"
#include "mark_internal.h"
#include "old_slots.h"

/* A reserved target is not an object until copying replaces this header. */
typedef struct PendingMove {
    struct PendingMove *next;
    void **entry;
} PendingMove;

_Static_assert(sizeof(PendingMove) == sizeof(ScoopObjectHeader) &&
                   _Alignof(PendingMove) <= _Alignof(ScoopObjectHeader),
               "pending move must fit in the unpublished target header");

typedef struct CopyTotals {
    uint64_t bytes;
    uint64_t objects;
} CopyTotals;

struct ScoopGcCopyBlock {
    ScoopGcCopyBlock *next;
    ScoopGcBlockMeta *block;
    PendingMove *first;
    PendingMove *last;
    CopyTotals totals;
};

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
    bool use_old_slots;
    bool parallel_copy;
    ScoopGcCopyBlock *copy_groups;
    ScoopGcOldSlotPlan slots;
} MovingPlan;

static void discard_copy_groups(MovingPlan *plan) {
    while (plan->copy_groups != NULL) {
        ScoopGcCopyBlock *group = plan->copy_groups;
        plan->copy_groups = group->next;
        group->block->copy_reserved = false;
        group->block->mark_index = SIZE_MAX;
        free(group);
    }
}

static bool append_move(MovingPlan *plan, PendingMove *move, ScoopGcBlockMeta *target) {
    PendingMove **first = &plan->first, **last = &plan->last;
    if (plan->parallel_copy) {
        if (!target->copy_reserved) {
            ScoopGcCopyBlock *group = calloc(1, sizeof *group);
            if (group == NULL) {
                return false;
            }
            group->block = target;
            group->next = plan->copy_groups;
            plan->copy_groups = group;
            target->copy_group = group;
            target->copy_reserved = true;
        }
        first = &target->copy_group->first;
        last = &target->copy_group->last;
    }
    if (*last == NULL) {
        *first = move;
    } else {
        (*last)->next = move;
    }
    *last = move;
    return true;
}

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
    scoop_heap_rollback_old_slots();
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
    size_t size = scoop_heap_small_object_size(object);
    ScoopGcBlockMeta *block = scoop_heap_known_object_block(object, size != 0);
    if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE ||
        (stress_move && object_pinned(block, ((uintptr_t)object - (uintptr_t)block_base(block)) /
                                                sizeof(uint64_t)))) {
        plan->survivors.objects += block->retained_young;
        return;
    }
    if (size == 0) {
        size = block->exact_size;
    }
    ScoopGcBlockMeta *target;
    size_t alignment = scoop_gc_object_alignment(object);
    void *destination = NULL;
    if (plan->survivor_capacity_limit != 0 && !block->survivor &&
        block->kind == SCOOP_BLOCK_KIND_SMALL) {
        destination = reserve_survivor(plan, size, alignment, &target);
    }
    if (destination == NULL) {
        if (plan->use_old_slots && size <= GC_OLD_SLOT_MAX) {
            destination = scoop_heap_reserve_old_slot(&plan->slots, size, &target);
        } else {
            destination = reserve_target(size, alignment, &target, plan->reuse_runs,
                                         plan->allow_growth);
        }
    }
    if (destination == NULL) {
        plan->reserved = false;
        return;
    }
    PendingMove *move = destination;
    *move = (PendingMove){.entry = entry};
    if (!append_move(plan, move, target)) {
        plan->reserved = false;
        return;
    }
    if (plan->reuse_runs && !target->region->evacuation_destination) {
        target->region->evacuation_destination = true;
        plan->target_regions++;
        plan->occupied_destinations += target->region->collection_live_bytes == 0;
    }
}

static CopyTotals copy_reserved_objects(PendingMove *move, ScoopGcBlockMeta *owned_block) {
    ScoopGcBlockMeta *target_block = owned_block;
    uintptr_t target_base = owned_block == NULL ? 0 : (uintptr_t)block_base(owned_block);
    CopyTotals totals = {0};
    while (move != NULL) {
        /* Copying destroys the temporary header, so take both fields first. */
        PendingMove *next = move->next;
        void **entry = move->entry;
        ScoopObjectHeader *source = *entry;
        size_t size = __atomic_load_n(&source->gc_word, __ATOMIC_RELAXED) & GC_SMALL_SIZE_MASK;
        ScoopGcBlockMeta *block;
        if (size == 0) {
            block = owned_block == NULL ? scoop_heap_known_object_block(move, false) : owned_block;
            size = block->exact_size;
        } else {
            if (target_block == NULL || (uintptr_t)move - target_base >= GC_BLOCK_SIZE) {
                target_block = scoop_heap_known_object_block(move, true);
                target_base = (uintptr_t)block_base(target_block);
            }
            block = target_block;
        }
        memcpy(move, source, size);
        totals.bytes += size;
        if (size <= GC_REGULAR_MAX) {
            scoop_heap_record_small_metadata(block, move, size, true, true);
        } else {
            publish_large_object(block, true);
        }
        __atomic_store_n(&source->gc_word, (uintptr_t)move | GC_FORWARDED_BIT, __ATOMIC_RELAXED);
        *entry = move;
        totals.objects++;
        move = next;
    }
    return totals;
}

void scoop_gc_copy_run_task(ScoopGcCopyBlock *group) {
    group->totals = copy_reserved_objects(group->first, group->block);
}

static void copy_plan(MovingPlan *plan) {
    CopyTotals totals = {0};
    if (plan->parallel_copy) {
        size_t owner = 0;
        for (ScoopGcCopyBlock *group = plan->copy_groups; group != NULL; group = group->next) {
            scoop_gc_mark_queue_push(&scoop_gc_marker.workers[owner], (ScoopGcMarkTask){
                .kind = SCOOP_COPY_BLOCK_TASK,
                .data.copy_group = group,
            });
            owner = (owner + 1) % scoop_gc_marker.active_workers;
        }
        if (plan->copy_groups != NULL) {
            scoop_gc_mark_pool_run(0);
        }
        for (ScoopGcCopyBlock *group = plan->copy_groups; group != NULL; group = group->next) {
            totals.bytes += group->totals.bytes;
            totals.objects += group->totals.objects;
        }
        discard_copy_groups(plan);
    } else {
        totals = copy_reserved_objects(plan->first, NULL);
    }
    scoop_gc_heap_state.copied_bytes += totals.bytes;
    moved_objects += totals.objects;
}

bool scoop_gc_heap_plan_moving_locked(bool minor, ScoopGcSurvivors *survivors) {
    *survivors = (ScoopGcSurvivors){0};
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    if (!minor && scoop_heap_uses_old_slots()) {
        scoop_gc_heap_state.metrics.last_full_source_regions = 0;
        scoop_gc_heap_state.metrics.last_full_target_regions = 0;
        return true;
    }
    bool ordinary_full = !minor && !stress_move;
    MovingPlan plan = {
        .reuse_runs = ordinary_full,
        .reserved = true,
        .use_old_slots = minor && scoop_heap_uses_old_slots(),
        .parallel_copy = scoop_gc_marker.active_workers > 1,
    };
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
        discard_copy_groups(&plan);
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
    copy_plan(&plan);
    scoop_gc_heap_state.metrics.copy_ns += scoop_gc_monotonic_ns() - copy_started;
    *survivors = plan.survivors;
    return true;
}
