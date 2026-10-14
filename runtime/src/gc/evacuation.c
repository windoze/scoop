/* Collector-only evacuation, forwarding and current-object traversal. */
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"

#if !SCOOP_PLATFORM_INLINE_GC_FORWARDING
#include "forwarding_internal.h"
#endif

void *scoop_heap_claim_relocated_object(void *object) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        heap_fatal("scan claimed for a non-object address");
    }
    void *current = scoop_heap_forward_object(object, block, word_index);
    if (current != object) {
        if (!object_meta(current, &block, &word_index)) {
            heap_fatal("scan claimed for a non-object address");
        }
        if (!object_marked(block, word_index) ||
            (block->state == SCOOP_BLOCK_EVACUATION_SOURCE && !object_pinned(block, word_index))) {
            heap_fatal("scan claimed for a non-current object");
        }
    }
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (block->large_scanned) {
            heap_fatal("marked object appeared twice in the retained live set");
        }
        block->large_scanned = true;
        return current;
    }
    if (bit_test(block->scanned, word_index)) {
        heap_fatal("marked object appeared twice in the retained live set");
    }
    bit_set(block->scanned, word_index);
    return current;
}

bool scoop_gc_object_was_scanned_locked(const void *object) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        return false;
    }
    return block->kind == SCOOP_BLOCK_KIND_LARGE ? block->large_scanned
                                                 : bit_test(block->scanned, word_index);
}

void scoop_gc_heap_verify_stress_moved_locked(void) {
    if (!collection_active || !stress_move) {
        heap_fatal("stress relocation verified outside a stress collection");
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || block->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked && !block->large_pinned) {
                void *old = (char *)block_base(block) + GC_LINE_SIZE;
                if (block->large_forwarding == NULL || block->large_forwarding == old) {
                    heap_fatal("stress collection left a live large object in place");
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK; word++) {
            if (!bit_test(block->starts, word) || !bit_test(block->marks, word) ||
                bit_test(block->pins, word)) {
                continue;
            }
            void *old = (char *)block_base(block) + word * sizeof(uint64_t);
            if (block->forwarding[word] == NULL || block->forwarding[word] == old) {
                heap_fatal("stress collection left a live small object in place");
            }
        }
    }
}

void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor, void *context) {
    if (visitor == NULL || !collection_active) {
        heap_fatal("invalid live-object visitor");
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block)) {
            continue;
        }
        if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked) {
                if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE && !block->large_pinned) {
                    if (block->large_forwarding == NULL) {
                        heap_fatal("live large source object was not forwarded");
                    }
                } else {
                    visitor((char *)block_base(block) + GC_LINE_SIZE, context);
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK; word++) {
            if (!bit_test(block->starts, word) || !bit_test(block->marks, word)) {
                continue;
            }
            if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE && !bit_test(block->pins, word)) {
                if (block->forwarding[word] == NULL) {
                    heap_fatal("live small source object was not forwarded");
                }
                continue;
            }
            visitor((char *)block_base(block) + word * sizeof(uint64_t), context);
        }
    }
}
