#ifndef SCOOP_GC_OBJECT_METADATA_H
#define SCOOP_GC_OBJECT_METADATA_H

#include "heap_internal.h"

/* The supplied block belongs to the address in this locked or immutable heap view. */
static inline size_t scoop_heap_object_word_index(ScoopGcBlockMeta *block, const void *object) {
    uintptr_t offset = (uintptr_t)object - (uintptr_t)block_base(block);
    return (size_t)(offset / sizeof(uint64_t));
}

/* The address has already been resolved to this ordinary block's range. */
static inline bool scoop_heap_small_object_meta_at(uintptr_t offset,
                                                   const ScoopGcBlockHead *head,
                                                   const uint64_t *starts, size_t *word_index) {
    if ((offset & (sizeof(void *) - 1)) != 0 || offset < GC_LINE_SIZE ||
        head->kind != SCOOP_BLOCK_KIND_SMALL || !scoop_heap_active_state(head->state)) {
        return false;
    }
    *word_index = (size_t)(offset / sizeof(uint64_t));
    uint64_t published = __atomic_load_n(&starts[*word_index / 64], __ATOMIC_ACQUIRE);
    return ((published >> (*word_index % 64)) & UINT64_C(1)) != 0;
}

static inline bool scoop_heap_object_meta_in_block(const void *object, ScoopGcBlockMeta *block,
                                                   size_t *word_index) {
    uintptr_t address = (uintptr_t)object;
    if (block == NULL) {
        return false;
    }
    uintptr_t base = (uintptr_t)block_base(block);
    if (block->head->kind == SCOOP_BLOCK_KIND_LARGE) {
        if (!active_head(block) || address != base + GC_LINE_SIZE ||
            !__atomic_load_n(&block->large_published, __ATOMIC_ACQUIRE)) {
            return false;
        }
        *word_index = GC_LINE_SIZE / sizeof(uint64_t);
        return true;
    }
    return scoop_heap_small_object_meta_at(address - base, block->head, block->starts, word_index);
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
