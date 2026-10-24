/* Exact object metadata, marking and pin state for the moving heap. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "allocation_metadata.h"

static size_t object_word_index(ScoopGcBlockMeta *block, const void *object) {
    uintptr_t offset = (uintptr_t)object - (uintptr_t)block_base(block);
    return (size_t)(offset / sizeof(uint64_t));
}

static inline bool object_meta_in_block(const void *object, ScoopGcBlockMeta *block,
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
        *word_index = object_word_index(block, object);
        uint64_t starts = __atomic_load_n(&block->starts[*word_index / 64], __ATOMIC_ACQUIRE);
        if (((starts >> (*word_index % 64)) & UINT64_C(1)) == 0) {
            return false;
        }
    }
    return true;
}

/* Read-only under the heap lock, or the coordinator's immutable STW mark view. */
bool object_meta(const void *object, ScoopGcBlockMeta **result, size_t *word_index) {
    ScoopGcBlockMeta *block = pointer_block(object);
    if (!object_meta_in_block(object, block, word_index)) {
        return false;
    }
    *result = block;
    return true;
}

bool scoop_heap_reference_meta(const void *object, ScoopGcBlockMeta **result, size_t *word_index) {
    ScoopGcBlockMeta *block = pointer_block(object);
    if (block == NULL) {
        return false;
    }
    if (!object_meta_in_block(object, block, word_index)) {
        heap_fatal("managed slot is not a current heap object start");
    }
    *result = block;
    return true;
}

bool scoop_gc_is_object_start_locked(const void *object) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    return object_meta(object, &block, &word_index);
}

size_t scoop_gc_object_size_locked(const void *object) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        heap_fatal("exact size requested for a non-object address");
    }
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        return block->exact_size;
    }
    uint16_t units = block->size_units[word_index];
    if (units == 0) {
        heap_fatal("published small object has no exact allocation size");
    }
    return (size_t)units * sizeof(uint64_t);
}

void record_small_object(ScoopGcBlockMeta *block, void *object, size_t exact_size, bool marked) {
    if (block->kind != SCOOP_BLOCK_KIND_SMALL || exact_size == 0 || exact_size > GC_REGULAR_MAX ||
        exact_size % sizeof(uint64_t) != 0) {
        heap_fatal("invalid small-object metadata publication");
    }
    size_t word = object_word_index(block, object);
    uintptr_t base = (uintptr_t)block_base(block);
    uintptr_t address = (uintptr_t)object;
    size_t line = (size_t)((address - base) / GC_LINE_SIZE);
    if (line == 0 || word >= GC_WORDS_PER_BLOCK || address + exact_size > base + GC_BLOCK_SIZE ||
        (__atomic_load_n(&block->starts[word / 64], __ATOMIC_RELAXED) &
         (UINT64_C(1) << (word % 64))) != 0) {
        heap_fatal("small object overlaps invalid storage");
    }
    scoop_heap_record_small_metadata(block, object, exact_size, marked, false);
}

void publish_large_object(ScoopGcBlockMeta *block, bool marked) {
    if (block->kind != SCOOP_BLOCK_KIND_LARGE || block->large_published || block->exact_size == 0) {
        heap_fatal("invalid large-object metadata publication");
    }
    block->large_marked = marked;
    if (marked) {
        block->live_bytes = block->exact_size;
        block->movable_live_bytes = block->exact_size;
    }
    __atomic_store_n(&block->large_published, true, __ATOMIC_RELEASE);
}

bool scoop_gc_update_pin_locked(const void *object, bool pinned) {
    ScoopGcBlockMeta *block;
    size_t word_index;
    if (!object_meta(object, &block, &word_index)) {
        heap_fatal("pin state changed for a non-object address");
    }
    bool previous;
    if (block->kind == SCOOP_BLOCK_KIND_LARGE) {
        previous = block->large_pinned;
        block->large_pinned = pinned;
    } else {
        previous = bit_test(block->pins, word_index);
        if (pinned) {
            bit_set(block->pins, word_index);
        } else {
            bit_clear(block->pins, word_index);
        }
    }
    ScoopObjectHeader *header = (ScoopObjectHeader *)object;
    if (pinned) {
        (void)__atomic_fetch_or(&header->gc_word, GC_PIN_BIT, __ATOMIC_ACQ_REL);
    } else {
        (void)__atomic_fetch_and(&header->gc_word, ~GC_PIN_BIT, __ATOMIC_ACQ_REL);
    }
    return previous;
}

bool object_pinned(const ScoopGcBlockMeta *block, size_t word) {
    return block->kind == SCOOP_BLOCK_KIND_LARGE ? block->large_pinned
                                                 : bit_test(block->pins, word);
}

bool object_marked(const ScoopGcBlockMeta *block, size_t word) {
    return block->kind == SCOOP_BLOCK_KIND_LARGE ? block->large_marked
                                                 : bit_test(block->marks, word);
}

bool scoop_gc_is_young_object_locked(const void *object) {
    ScoopGcBlockMeta *block = pointer_block(object);
    return block != NULL && block->generation == SCOOP_GC_YOUNG;
}

size_t scoop_gc_heap_begin_collection_locked(bool minor) {
    require_heap();
    if (collection_active) {
        heap_fatal("nested heap collection");
    }
    if (!minor) {
        free_run_nodes();
        scoop_gc_heap_state.old_cursor = NULL;
        scoop_gc_heap_state.old_limit = NULL;
    }
    evacuation_block = NULL;
    evacuation_cursor = NULL;
    evacuation_limit = NULL;
    moved_objects = 0;
    size_t mark_blocks = 0;
    for (ScoopGcBlockMeta *block = scoop_heap_first_block(); block != NULL;
         block = scoop_heap_next_block(block)) {
        if (!active_head(block) || (minor && block->generation == SCOOP_GC_OLD)) {
            continue;
        }
        if (block->state == SCOOP_BLOCK_EVACUATION_SOURCE ||
            block->state == SCOOP_BLOCK_EVACUATION_TARGET) {
            heap_fatal("unfinished evacuation state at collection start");
        }
        block->live_bytes = 0;
        block->movable_live_bytes = 0;
        block->mark_index = mark_blocks++;
        block->large_forwarding = NULL;
        block->large_marked = false;
        block->large_scanned = false;
        free(block->forwarding);
        block->forwarding = NULL;
        if (block->kind == SCOOP_BLOCK_KIND_SMALL) {
            memset(block->marks, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
            memset(block->scanned, 0, GC_BITMAP_WORDS * sizeof(uint64_t));
            memset(block->line_live, 0, GC_LINE_BITMAP_WORDS * sizeof(uint64_t));
        }
    }
    collection_active = true;
    return mark_blocks;
}

bool scoop_gc_is_published_object(const void *object) {
    if (object == NULL) {
        return false;
    }

    /* Collection takes these locks in heap -> roots order while classifying
     * stable targets.  Keep the same order here so a NoGC runtime entry can
     * validate a throw source without racing root-table mutation. */
    scoop_gc_heap_lock();
    bool published = scoop_gc_is_object_start_locked(object);
    scoop_gc_roots_lock();
    published = published || scoop_gc_is_immortal_object_locked(object);
    scoop_gc_roots_unlock();
    scoop_gc_heap_unlock();
    return published;
}
