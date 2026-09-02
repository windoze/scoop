/* Mutator allocation paths: owner-only TLAB bump/refill and large objects. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "gc_internal.h"
#include "heap_internal.h"
#include "../thread.h"

static size_t normalize_allocation_size(size_t size) {
    if (size < sizeof(ScoopObjectHeader)) {
        size = sizeof(ScoopObjectHeader);
    }
    if (size > SIZE_MAX - (sizeof(uint64_t) - 1)) {
        heap_fatal("allocation size overflows normalization");
    }
    return (size + sizeof(uint64_t) - 1) &
           ~(sizeof(uint64_t) - 1);
}

static void *tlab_allocate(ScoopThreadState *thread, size_t size) {
    char *cursor = thread->allocation.cursor;
    if (cursor == NULL) {
        return NULL;
    }
    char *line_end =
        (char *)(((uintptr_t)cursor & ~(uintptr_t)(GC_LINE_SIZE - 1)) +
                 GC_LINE_SIZE);
    char *candidate = cursor + size <= line_end ? cursor : line_end;
    if (candidate + size > thread->allocation.limit) {
        return NULL;
    }
    thread->allocation.cursor = candidate + size;
    return candidate;
}

static bool refill_tlab(ScoopThreadState *thread) {
    lock_heap();
    ScoopGcFreeRun *run = free_runs;
    if (run != NULL) {
        free_runs = run->next;
        thread->allocation.cursor =
            (char *)block_base(run->block_index) +
            (size_t)run->first_line * GC_LINE_SIZE;
        thread->allocation.limit = thread->allocation.cursor +
                                   (size_t)run->line_count * GC_LINE_SIZE;
        free(run);
        unlock_heap();
        return true;
    }
    if (committed_bytes + GC_BLOCK_SIZE > collection_threshold) {
        unlock_heap();
        return false;
    }
    uint32_t index = activate_small_block(SCOOP_BLOCK_MUTATOR);
    if (index != UINT32_MAX) {
        thread->allocation.cursor =
            (char *)block_base(index) + GC_LINE_SIZE;
        thread->allocation.limit =
            (char *)block_base(index) + GC_BLOCK_SIZE;
    }
    unlock_heap();
    return index != UINT32_MAX;
}

static void *allocate_small(ScoopThreadState *thread, size_t size) {
    bool collected = false;
    for (;;) {
        void *object = tlab_allocate(thread, size);
        if (object != NULL) {
            return object;
        }
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        if (refill_tlab(thread)) {
            continue;
        }
        if (collected) {
            heap_fatal("GC arena exhausted allocating a small object");
        }
        scoop_gc_collect_internal();
        collected = true;
    }
}

static void *allocate_small_stress(size_t size) {
    lock_heap();
    uint32_t index = activate_small_block(SCOOP_BLOCK_MUTATOR);
    unlock_heap();
    if (index == UINT32_MAX) {
        heap_fatal(
            "stress arena exhausted by permanently quarantined blocks");
    }
    void *object = (char *)block_base(index) + GC_LINE_SIZE;
    if ((char *)object + size >
        (char *)block_base(index) + GC_BLOCK_SIZE) {
        heap_fatal("stress small allocation exceeds its block");
    }
    return object;
}

static void *allocate_large(size_t size, uint32_t *block_index) {
    if (size > GC_ARENA_SIZE - GC_LINE_SIZE) {
        heap_fatal("large object exceeds the GC arena");
    }
    bool collected = false;
    for (;;) {
        lock_heap();
        size_t span = (GC_LINE_SIZE + size + GC_BLOCK_SIZE - 1) &
                      ~(GC_BLOCK_SIZE - 1);
        bool threshold_exceeded =
            !collected && committed_bytes + span > collection_threshold;
        uint32_t index = threshold_exceeded
                             ? UINT32_MAX
                             : activate_large_block(size,
                                                    SCOOP_BLOCK_MUTATOR);
        unlock_heap();
        if (index != UINT32_MAX) {
            *block_index = index;
            return (char *)block_base(index) + GC_LINE_SIZE;
        }
        if (collected) {
            heap_fatal("GC arena exhausted allocating a large object");
        }
        scoop_gc_collect_internal();
        collected = true;
    }
}

static void *allocate_large_stress(size_t size, uint32_t *block_index) {
    lock_heap();
    uint32_t index =
        activate_large_block(size, SCOOP_BLOCK_MUTATOR);
    unlock_heap();
    if (index == UINT32_MAX) {
        heap_fatal(
            "stress arena exhausted by permanently quarantined blocks");
    }
    *block_index = index;
    return (char *)block_base(index) + GC_LINE_SIZE;
}

void scoop_runtime_finish_tlab_alloc(void *object,
                                     const ScoopTypeDescriptor *td,
                                     size_t size) {
    if (scoop_gc_stress_move_enabled()) {
        heap_fatal("inline TLAB allocation remained enabled in stress mode");
    }
    size = normalize_allocation_size(size);
    uint32_t block_index;
    if (object == NULL || td == NULL || size > GC_SMALL_MAX ||
        !pointer_block_index(object, &block_index)) {
        heap_fatal("invalid inline TLAB allocation");
    }
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    record_small_object(block_index, object, size, false);
    atomic_fetch_add_explicit(&live_objects, 1, memory_order_relaxed);
}

void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size) {
    if (td == NULL) {
        heap_fatal("allocation has no TypeDescriptor");
    }
    ScoopThreadState *thread = scoop_thread_current_required();
    size = normalize_allocation_size(size);
    bool stress = scoop_gc_stress_move_enabled();
    if (stress) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        scoop_gc_collect_internal();
        if (thread->allocation.cursor != NULL ||
            thread->allocation.limit != NULL) {
            heap_fatal("stress collection published a mutator TLAB");
        }
    }
    if (size <= GC_SMALL_MAX) {
        void *object = stress ? allocate_small_stress(size)
                              : allocate_small(thread, size);
        if (stress) {
            memset(object, 0, size);
            ScoopObjectHeader *header = object;
            header->td = td;
            header->gc_word = 0;
            uint32_t block_index;
            if (!pointer_block_index(object, &block_index)) {
                heap_fatal("stress allocation lies outside the arena");
            }
            record_small_object(block_index, object, size, false);
            atomic_fetch_add_explicit(&live_objects, 1,
                                      memory_order_relaxed);
        } else {
            scoop_runtime_finish_tlab_alloc(object, td, size);
        }
        return object;
    }
    uint32_t block_index;
    void *object = stress ? allocate_large_stress(size, &block_index)
                          : allocate_large(size, &block_index);
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    publish_large_object(block_index, false);
    atomic_fetch_add_explicit(&live_objects, 1, memory_order_relaxed);
    return object;
}
