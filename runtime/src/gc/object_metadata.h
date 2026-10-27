#ifndef SCOOP_GC_OBJECT_METADATA_H
#define SCOOP_GC_OBJECT_METADATA_H

#include "heap_internal.h"

/* The supplied block belongs to the address in this locked or immutable heap view. */
static inline size_t scoop_heap_object_word_index(ScoopGcBlockMeta *block, const void *object) {
    uintptr_t offset = (uintptr_t)object - (uintptr_t)block_base(block);
    return (size_t)(offset / sizeof(uint64_t));
}

static inline bool scoop_heap_object_meta_in_block(const void *object, ScoopGcBlockMeta *block,
                                                   size_t *word_index) {
    uintptr_t address = (uintptr_t)object;
    if ((address & (sizeof(void *) - 1)) != 0) {
        return false;
    }
    if (block == NULL || !active_head(block)) {
        return false;
    }
    uintptr_t base = (uintptr_t)block_base(block);
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (!__atomic_load_n(&block->large_published, __ATOMIC_ACQUIRE) ||
            address != base + GC_LINE_SIZE) {
            return false;
        }
        *word_index = GC_LINE_SIZE / sizeof(uint64_t);
    } else {
        if (block->kind != SCOOP_BLOCK_KIND_SMALL || address < base + GC_LINE_SIZE) {
            return false;
        }
        *word_index = scoop_heap_object_word_index(block, object);
        uint64_t starts = __atomic_load_n(&block->starts[*word_index / 64], __ATOMIC_ACQUIRE);
        if (((starts >> (*word_index % 64)) & UINT64_C(1)) == 0) {
            return false;
        }
    }
    return true;
}

static inline bool scoop_heap_reference_in_block(const void *object, ScoopGcBlockMeta *block,
                                                size_t *word_index) {
    if (block == NULL) {
        return false;
    }
    if (!scoop_heap_object_meta_in_block(object, block, word_index)) {
        heap_fatal("managed slot is not a current heap object start");
    }
    return true;
}

#endif
