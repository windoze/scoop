/* Collector-only evacuation, forwarding and current-object traversal. */
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"

#if SCOOP_GC_TRACK_RELOCATION && !SCOOP_PLATFORM_INLINE_GC_FORWARDING
#include "forwarding_internal.h"
#endif

#if SCOOP_GC_TRACK_RELOCATION
void scoop_heap_claim_current_object(void *object) {
    if (!collection_active) {
        heap_fatal("scan claimed outside collection");
    }
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        heap_fatal("scan claimed for a non-object address");
    }
    if (!object_marked(block, word_index) ||
        (block->head->state == SCOOP_BLOCK_EVACUATION_SOURCE && !object_pinned(block, word_index))) {
        heap_fatal("scan claimed for a non-current object");
    }
    if (block->head->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (__atomic_exchange_n(&block->large_scanned, true, __ATOMIC_RELAXED)) {
            heap_fatal("marked object appeared twice in the retained live set");
        }
        return;
    }
    uint64_t mask = UINT64_C(1) << (word_index % 64);
    if (__atomic_fetch_or(&block->scanned[word_index / 64], mask, __ATOMIC_RELAXED) & mask) {
        heap_fatal("marked object appeared twice in the retained live set");
    }
}

bool scoop_gc_object_was_scanned_locked(const void *object) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        return false;
    }
    return block->head->kind == SCOOP_BLOCK_KIND_LARGE ? block->large_scanned
                                                 : bit_test(block->scanned, word_index);
}
#endif

#if SCOOP_ENABLE_GC_STRESS
void scoop_gc_heap_verify_stress_moved_locked(void) {
    if (!collection_active || !stress_move) {
        heap_fatal("stress relocation verified outside a stress collection");
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || block->head->state != SCOOP_BLOCK_EVACUATION_SOURCE) {
            continue;
        }
        if (block->head->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked && !block->large_pinned) {
                void *old = (char *)block_base(block) + GC_LINE_SIZE;
                void *forwarded = scoop_heap_forwarding_address(old);
                if (forwarded == NULL || forwarded == old) {
                    heap_fatal("stress collection left a live large object in place");
                }
            }
            continue;
        }
        for (size_t word = GC_LINE_SIZE / sizeof(uint64_t); word < GC_WORDS_PER_BLOCK; word++) {
            if (!bit_test(block->starts, word) || !bit_test(block->marks, word) ||
                object_pinned(block, word)) {
                continue;
            }
            void *old = (char *)block_base(block) + word * sizeof(uint64_t);
            void *forwarded = scoop_heap_forwarding_address(old);
            if (forwarded == NULL || forwarded == old) {
                heap_fatal("stress collection left a live small object in place");
            }
        }
    }
}
#endif

#if SCOOP_GC_TRACK_RELOCATION
void scoop_gc_visit_current_objects_locked(ScoopGcHeapObjectVisitor visitor, void *context) {
    if (visitor == NULL || !collection_active) {
        heap_fatal("invalid live-object visitor");
    }
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block)) {
            continue;
        }
        if (block->head->kind == SCOOP_BLOCK_KIND_LARGE) {
            if (block->large_marked) {
                if (block->head->state == SCOOP_BLOCK_EVACUATION_SOURCE && !block->large_pinned) {
                    void *object = (char *)block_base(block) + GC_LINE_SIZE;
                    if (scoop_heap_forwarding_address(object) == NULL) {
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
            if (block->head->state == SCOOP_BLOCK_EVACUATION_SOURCE && !object_pinned(block, word)) {
                void *object = (char *)block_base(block) + word * sizeof(uint64_t);
                if (scoop_heap_forwarding_address(object) == NULL) {
                    heap_fatal("live small source object was not forwarded");
                }
                continue;
            }
            visitor((char *)block_base(block) + word * sizeof(uint64_t), context);
        }
    }
}
#endif
