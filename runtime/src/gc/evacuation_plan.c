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

static bool append_move(PlannedMove **moves, size_t *count, size_t *capacity,
                        ScoopGcBlockMeta *block, size_t word, bool reuse_runs, bool allow_growth) {
    if (*count == *capacity) {
        size_t next = *capacity == 0 ? 256 : *capacity * 2;
        PlannedMove *grown = realloc(*moves, next * sizeof *grown);
        if (grown == NULL) {
            heap_fatal("out of memory planning evacuation");
        }
        *moves = grown;
        *capacity = next;
    }
    void *source = (char *)block_base(block) + word * sizeof(uint64_t);
    size_t size = scoop_gc_object_size_locked(source);
    const ScoopTypeDescriptor *td = ((ScoopObjectHeader *)source)->td;
    ScoopGcBlockMeta *target;
    void *destination = reserve_target(size, (size_t)td->instance_shape.instance_alignment, &target,
                                       reuse_runs, allow_growth);
    if (destination == NULL) {
        return false;
    }
    (*moves)[(*count)++] = (PlannedMove){source, destination, size, word, block, target};
    return true;
}

bool scoop_gc_heap_plan_moving_locked(bool minor) {
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    size_t source_regions = scoop_heap_select_evacuation_sources(minor);
    bool ordinary_full = !minor && !stress_move;
    if (ordinary_full && source_regions == 0) {
        return true;
    }
    if (ordinary_full) {
        scoop_heap_prepare_evacuation_targets();
    }
    bool allow_growth = !ordinary_full || source_regions > 1;
    PlannedMove *moves = NULL;
    size_t count = 0, capacity = 0;
    bool reserved = true;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); reserved && block != NULL;
         block = scoop_heap_next_block(block)) {
        if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            reserved = append_move(&moves, &count, &capacity, block,
                                   GC_LINE_SIZE / sizeof(uint64_t), ordinary_full, allow_growth);
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); reserved && word < GC_WORDS_PER_BLOCK;
             word++) {
            if (bit_test(block->starts, word) && bit_test(block->marks, word) &&
                !bit_test(block->pins, word)) {
                reserved = append_move(&moves, &count, &capacity, block, word, ordinary_full,
                                       allow_growth);
            }
        }
    }
    if (ordinary_full) {
        size_t occupied_destinations = 0;
        for (size_t index = 0; index < count; index++) {
            ScoopGcRegion *region = moves[index].target_block->region;
            if (region->collection_live_bytes == 0 && !region->evacuation_destination) {
                region->evacuation_destination = true;
                occupied_destinations++;
            }
        }
        reserved &= occupied_destinations < source_regions;
    }
    if (!reserved) {
        free(moves);
        rollback_reservations();
        if (stress_move) {
            heap_fatal("stress heap exhausted during evacuation reservation");
        }
        return false;
    }
    for (size_t index = 0; index < count; index++) {
        PlannedMove *move = &moves[index];
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
    free(moves);
    return true;
}
