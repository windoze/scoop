#ifndef SCOOP_GC_ALLOCATION_METADATA_H
#define SCOOP_GC_ALLOCATION_METADATA_H

#include "heap_internal.h"

static inline void scoop_heap_publish_allocation_bits(uint64_t *word, uint64_t mask, int order,
                                                      bool owned) {
    if (owned) {
        uint64_t previous = __atomic_load_n(word, __ATOMIC_RELAXED);
        __atomic_store_n(word, previous | mask, order);
    } else {
        (void)__atomic_fetch_or(word, mask, order);
    }
}

/* The caller has reserved a valid, aligned small-object range. Nursery allocation
 * and STW copying have one writer per block; shared allocation does not. */
static inline void scoop_heap_record_small_metadata(ScoopGcBlockMeta *block, void *object,
                                                    size_t size, bool marked, bool owned) {
    size_t offset = (uintptr_t)object - (uintptr_t)scoop_heap_block_base(block);
    size_t word = offset / sizeof(uint64_t);
    if (!marked) {
        ScoopObjectHeader *header = object;
        header->gc_word = (header->gc_word & ~GC_SMALL_SIZE_MASK) | size;
    }
    if (marked) {
        size_t first = offset / GC_LINE_SIZE;
        size_t last = (offset + size - 1) / GC_LINE_SIZE;
        for (size_t line = first; line <= last; line++) {
            uint64_t mask = UINT64_C(1) << (line % 64);
            block->line_live[line / 64] |= mask;
        }
        block->marks[word / 64] |= UINT64_C(1) << (word % 64);
        block->live_bytes += size;
        block->movable_live_bytes += size;
    }
    scoop_heap_publish_allocation_bits(&block->starts[word / 64], UINT64_C(1) << (word % 64),
                                       __ATOMIC_RELEASE, owned);
}

#endif
