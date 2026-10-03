/* Collector-only evacuation, forwarding and current-object traversal. */
#include <stdlib.h>
#include <string.h>

#include "../value_shape.h"
#include "gc_internal.h"
#include "heap_internal.h"

static size_t usable_bytes(const ScoopGcBlockMeta *block) {
    return block->kind == SCOOP_BLOCK_KIND_SMALL
               ? GC_BLOCK_SIZE - GC_LINE_SIZE
               : (size_t)block->span_blocks * GC_BLOCK_SIZE - GC_LINE_SIZE;
}

static void select_source(uint32_t index) {
    ScoopGcBlockMeta *source = &blocks[index];
    source->state = SCOOP_BLOCK_EVACUATION_SOURCE;
    if (source->kind == SCOOP_BLOCK_KIND_SMALL) {
        source->forwarding = calloc(GC_WORDS_PER_BLOCK, sizeof(void *));
        if (source->forwarding == NULL) {
            heap_fatal("out of memory allocating forwarding metadata");
        }
    }
}

void scoop_gc_heap_plan_moving_locked(void) {
    if (!collection_active) {
        heap_fatal("evacuation planned outside collection");
    }
    if (stress_move) {
        for (uint32_t index = 0; index < arena_next_block; index++) {
            ScoopGcBlockMeta *block = &blocks[index];
            if (active_head(block) && block->state != SCOOP_BLOCK_EVACUATION_TARGET &&
                block->movable_live_bytes != 0) {
                select_source(index);
            }
        }
        return;
    }

    uint32_t selected = UINT32_MAX;
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block) || block->state == SCOOP_BLOCK_EVACUATION_TARGET ||
            block->movable_live_bytes == 0) {
            continue;
        }
        if (selected == UINT32_MAX) {
            selected = index;
            continue;
        }
        ScoopGcBlockMeta *best = &blocks[selected];
        if (block->live_bytes * usable_bytes(best) <
            best->live_bytes * usable_bytes(block)) {
            selected = index;
        }
    }
    if (selected == UINT32_MAX) {
        return;
    }
    select_source(selected);
}

static void *evacuate_allocate_small(size_t size, size_t alignment,
                                     uint32_t *block_index) {
    for (;;) {
        if (evacuation_cursor != NULL) {
            evacuation_cursor =
                (char *)scoop_shape_align((uintptr_t)evacuation_cursor, alignment);
            char *line_end = (char *)(((uintptr_t)evacuation_cursor &
                                       ~(uintptr_t)(GC_LINE_SIZE - 1)) +
                                      GC_LINE_SIZE);
            char *candidate =
                evacuation_cursor + size <= line_end ? evacuation_cursor : line_end;
            if (candidate + size <= evacuation_limit) {
                evacuation_cursor = candidate + size;
                *block_index = evacuation_block;
                return candidate;
            }
        }
        evacuation_block = activate_small_block(SCOOP_BLOCK_EVACUATION_TARGET);
        if (evacuation_block == UINT32_MAX) {
            heap_fatal(stress_move
                           ? "stress arena exhausted by permanently quarantined blocks "
                             "during small-object evacuation"
                           : "to-space exhausted while evacuating small objects");
        }
        evacuation_cursor = (char *)block_base(evacuation_block) + GC_LINE_SIZE;
        evacuation_limit = (char *)block_base(evacuation_block) + GC_BLOCK_SIZE;
    }
}

static void *evacuate_allocate(size_t size, size_t alignment, uint32_t *block_index) {
    if (size <= GC_SMALL_MAX) {
        return evacuate_allocate_small(size, alignment, block_index);
    }
    uint32_t index = activate_large_block(size, SCOOP_BLOCK_EVACUATION_TARGET);
    if (index == UINT32_MAX) {
        heap_fatal(stress_move ? "stress arena exhausted by permanently quarantined "
                                 "blocks during large-object evacuation"
                               : "to-space exhausted while evacuating a large object");
    }
    *block_index = index;
    return (char *)block_base(index) + GC_LINE_SIZE;
}

void *scoop_gc_forward_object_locked(void *object) {
    if (!collection_active) {
        heap_fatal("object forwarded outside collection");
    }
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("forward requested for a non-object address");
    }
    ScoopGcBlockMeta *source = &blocks[block_index];
    if (!object_marked(source, word_index)) {
        heap_fatal("unmarked object reached relocation");
    }
    if (source->state != SCOOP_BLOCK_EVACUATION_SOURCE ||
        object_pinned(source, word_index)) {
        return object;
    }
    void *forwarded = source->kind == SCOOP_BLOCK_KIND_LARGE
                          ? source->large_forwarding
                          : source->forwarding[word_index];
    if (forwarded != NULL) {
        return forwarded;
    }
    size_t size = scoop_gc_object_size_locked(object);
    uint32_t target_index;
    const ScoopTypeDescriptor *td = ((const ScoopObjectHeader *)object)->td;
    forwarded = evacuate_allocate(size, (size_t)td->instance_shape.instance_alignment,
                                  &target_index);
    memcpy(forwarded, object, size);
    if (size <= GC_SMALL_MAX) {
        record_small_object(target_index, forwarded, size, true);
    } else {
        publish_large_object(target_index, true);
    }
    if (source->kind == SCOOP_BLOCK_KIND_LARGE) {
        source->large_forwarding = forwarded;
    } else {
        source->forwarding[word_index] = forwarded;
    }
    moved_objects++;
    return forwarded;
}

bool scoop_gc_claim_object_scan_locked(void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        heap_fatal("scan claimed for a non-object address");
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    bool current = object_marked(block, word_index) &&
                   !(block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                     !object_pinned(block, word_index));
    if (!current) {
        heap_fatal("scan claimed for a non-current object");
    }
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (block->large_scanned) {
            return false;
        }
        block->large_scanned = true;
        return true;
    }
    if (bit_test(block->scanned, word_index)) {
        return false;
    }
    bit_set(block->scanned, word_index);
    return true;
}

bool scoop_gc_object_was_scanned_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    return block->kind == SCOOP_BLOCK_KIND_LARGE ? block->large_scanned
                                                 : bit_test(block->scanned, word_index);
}

bool scoop_gc_is_forwarded_old_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    if (block->state != SCOOP_BLOCK_EVACUATION_SOURCE ||
        object_pinned(block, word_index)) {
        return false;
    }
    return block->kind == SCOOP_BLOCK_KIND_LARGE
               ? block->large_forwarding != NULL
               : block->forwarding[word_index] != NULL;
}

bool scoop_gc_is_current_live_object_locked(const void *object) {
    uint32_t block_index;
    size_t word_index;
    if (!object_meta(object, &block_index, &word_index)) {
        return false;
    }
    ScoopGcBlockMeta *block = &blocks[block_index];
    return object_marked(block, word_index) &&
           !(block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
             !object_pinned(block, word_index));
}

void scoop_gc_heap_verify_stress_moved_locked(void) {
    if (!collection_active || !stress_move) {
        heap_fatal("stress relocation verified outside a stress collection");
    }
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block) || block->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked && !block->large_pinned) {
                void *old = (char *)block_base(index) + GC_LINE_SIZE;
                if (block->large_forwarding == NULL || block->large_forwarding == old) {
                    heap_fatal("stress collection left a live large object in place");
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK;
             word++) {
            if (!bit_test(block->starts, word) || !bit_test(block->marks, word) ||
                bit_test(block->pins, word)) {
                continue;
            }
            void *old = (char *)block_base(index) + word * sizeof(uint64_t);
            if (block->forwarding[word] == NULL || block->forwarding[word] == old) {
                heap_fatal("stress collection left a live small object in place");
            }
        }
    }
}

void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor,
                                           void *context) {
    if (visitor == NULL || !collection_active) {
        heap_fatal("invalid live-object visitor");
    }
    for (uint32_t index = 0; index < arena_next_block; index++) {
        ScoopGcBlockMeta *block = &blocks[index];
        if (!active_head(block)) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked) {
                if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                    !block->large_pinned) {
                    if (block->large_forwarding == NULL) {
                        heap_fatal("live large source object was not forwarded");
                    }
                } else {
                    visitor((char *)block_base(index) + GC_LINE_SIZE, context);
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK;
             word++) {
            if (!bit_test(block->starts, word) || !bit_test(block->marks, word)) {
                continue;
            }
            if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE &&
                !bit_test(block->pins, word)) {
                if (block->forwarding[word] == NULL) {
                    heap_fatal("live small source object was not forwarded");
                }
                continue;
            }
            visitor((char *)block_base(index) + word * sizeof(uint64_t), context);
        }
    }
}
