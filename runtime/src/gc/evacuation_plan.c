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

static bool block_has_pins(const ScoopGcBlockMeta *block) {
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

static void select_source(ScoopGcBlockMeta *source) {
    source->state = SCOOP_BLOCK_EVACUATION_SOURCE;
    if (source->kind == SCOOP_BLOCK_KIND_SMALL) {
        source->forwarding = calloc(GC_WORDS_PER_BLOCK, sizeof(void *));
        if (source->forwarding == NULL) {
            heap_fatal("out of memory allocating forwarding metadata");
        }
    }
}

static void select_sources(bool minor) {
    ScoopGcBlockMeta *selected = NULL;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || block->movable_live_bytes == 0) {
            continue;
        }
        if (minor) {
            if (block->generation == SCOOP_GC_YOUNG && !block_has_pins(block)) {
                select_source(block);
            }
        } else if (stress_move) {
            select_source(block);
        } else if (selected == NULL ||
                   (double)block->live_bytes / (double)scoop_heap_block_bytes(block) <
                       (double)selected->live_bytes / (double)scoop_heap_block_bytes(selected)) {
            selected = block;
        }
    }
    if (!minor && !stress_move && selected != NULL) {
        select_source(selected);
    }
}

static void *reserve_target(size_t size, size_t alignment, ScoopGcBlockMeta **block) {
    if (size > GC_REGULAR_MAX) {
        *block = activate_large_block(size, SCOOP_BLOCK_EVACUATION_TARGET);
        return *block == NULL ? NULL : (char *)block_base(*block) + GC_LINE_SIZE;
    }
    void *object = scoop_heap_bump(&evacuation_cursor, evacuation_limit, size, alignment);
    if (object == NULL) {
        evacuation_block = activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET);
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
            block->state = block_has_pins(block) ? SCOOP_BLOCK_PINNED_PARTIAL : SCOOP_BLOCK_MUTATOR;
            free(block->forwarding);
            block->forwarding = NULL;
        }
    }
    evacuation_block = NULL;
    evacuation_cursor = evacuation_limit = NULL;
}

static bool append_move(PlannedMove **moves, size_t *count, size_t *capacity,
                        ScoopGcBlockMeta *block, size_t word) {
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
    void *destination =
        reserve_target(size, (size_t)td->instance_shape.instance_alignment, &target);
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
    select_sources(minor);
    PlannedMove *moves = NULL;
    size_t count = 0, capacity = 0;
    bool reserved = true;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); reserved && block != NULL;
         block = scoop_heap_next_block(block)) {
        if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            reserved =
                append_move(&moves, &count, &capacity, block, GC_LINE_SIZE / sizeof(uint64_t));
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); reserved && word < GC_WORDS_PER_BLOCK;
             word++) {
            if (bit_test(block->starts, word) && bit_test(block->marks, word) &&
                !bit_test(block->pins, word)) {
                reserved = append_move(&moves, &count, &capacity, block, word);
            }
        }
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
