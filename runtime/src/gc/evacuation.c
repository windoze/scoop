/* Collector-only evacuation, forwarding and current-object traversal. */
#include <stdlib.h>
#include <string.h>

#include "../value_shape.h"
#include "gc_internal.h"
#include "heap_internal.h"

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
    heap_fatal("marked evacuation source has no planned forwarding address");
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
