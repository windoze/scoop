/* Mutator allocation paths: owner-only TLAB bump/refill and large objects. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "../thread.h"
#include "../thread/testing.h"
#include "../value_shape.h"
#include "gc_internal.h"
#include "heap_internal.h"

_Static_assert(GC_LINE_SIZE % SCOOP_MAXIMUM_MANAGED_ALIGNMENT == 0,
               "GC line starts must satisfy every managed alignment");
_Static_assert(GC_REGULAR_MAX / sizeof(uint64_t) <= UINT16_MAX,
               "regular object sizes must fit exact side metadata");

void *scoop_heap_bump(char **cursor, char *limit, size_t size, size_t alignment) {
    if (*cursor == NULL) {
        return NULL;
    }
    char *object = (char *)scoop_shape_align((uintptr_t)*cursor, alignment);
    if (object > limit || size > (size_t)(limit - object)) {
        return NULL;
    }
    *cursor = object + size;
    return object;
}

ScoopGcBlockMeta *scoop_heap_take_free_run(size_t size, char **cursor, char **limit) {
    ScoopGcFreeRun **available = &free_runs;
    while (*available != NULL && ((*available)->block->region->evacuation_source ||
                                  (size_t)(*available)->line_count * GC_LINE_SIZE < size)) {
        available = &(*available)->next;
    }
    ScoopGcFreeRun *run = *available;
    if (run == NULL) {
        return NULL;
    }
    *available = run->next;
    *cursor = (char *)block_base(run->block) + (size_t)run->first_line * GC_LINE_SIZE;
    *limit = *cursor + (size_t)run->line_count * GC_LINE_SIZE;
    ScoopGcBlockMeta *block = run->block;
    block->discard_pending = true;
    free(run);
    return block;
}

typedef enum RefillResult {
    REFILL_READY,
    REFILL_MINOR,
    REFILL_FULL,
} RefillResult;

static RefillResult refill_nursery(ScoopThreadState *thread, bool after_full) {
    lock_heap();
    size_t capacity = scoop_gc_heap_state.stress_minor ? GC_BLOCK_SIZE : GC_NURSERY_CAPACITY;
    if (!after_full && committed_bytes + GC_BLOCK_SIZE > collection_threshold) {
        unlock_heap();
        return REFILL_FULL;
    }
    if (scoop_gc_heap_state.nursery_bytes >= capacity) {
        unlock_heap();
        return REFILL_MINOR;
    }
    ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    if (block == NULL) {
        unlock_heap();
        return REFILL_FULL;
    }
    block->generation = SCOOP_GC_YOUNG;
    scoop_gc_heap_state.nursery_bytes += GC_BLOCK_SIZE;
    thread->allocation_block = block;
    thread->allocation.cursor = (char *)block_base(block) + GC_LINE_SIZE;
    thread->allocation.limit = (char *)block_base(block) + GC_BLOCK_SIZE;
    unlock_heap();
    return REFILL_READY;
}

static void *allocate_small(ScoopThreadState *thread, size_t size, size_t alignment) {
    bool full_attempted = false;
    for (;;) {
        void *object =
            scoop_heap_bump(&thread->allocation.cursor, thread->allocation.limit, size, alignment);
        if (object != NULL) {
            return object;
        }
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        thread->allocation_block = NULL;
        RefillResult refill = refill_nursery(thread, full_attempted);
        if (refill == REFILL_READY) {
            continue;
        }
        if (refill == REFILL_MINOR) {
            /* Another mutator can claim the newly emptied nursery before
             * this thread resumes. Capacity contention is not heap OOM. */
            scoop_gc_collect_minor_internal();
            full_attempted = false;
        } else if (!full_attempted) {
            full_attempted = scoop_gc_collect_internal();
        } else {
            heap_fatal("GC heap exhausted allocating a regular object");
        }
        SCOOP_THREAD_TEST_POINT(SCOOP_TEST_NURSERY_RETRY);
    }
}

static void *allocate_old(size_t size, size_t alignment) {
    bool collected = false;
    for (;;) {
        lock_heap();
        char **cursor = &scoop_gc_heap_state.old_cursor;
        char **limit = &scoop_gc_heap_state.old_limit;
        void *object = scoop_heap_bump(cursor, *limit, size, alignment);
        if (object == NULL && scoop_heap_take_free_run(size, cursor, limit)) {
            object = scoop_heap_bump(cursor, *limit, size, alignment);
        }
        if (object == NULL &&
            (collected || committed_bytes + GC_BLOCK_SIZE <= collection_threshold)) {
            ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
            if (block != NULL) {
                *cursor = (char *)block_base(block) + GC_LINE_SIZE;
                *limit = (char *)block_base(block) + GC_BLOCK_SIZE;
                object = scoop_heap_bump(cursor, *limit, size, alignment);
            }
        }
        unlock_heap();
        if (object != NULL) {
            return object;
        }
        if (collected) {
            heap_fatal("GC heap exhausted allocating a pretenured object");
        }
        collected = scoop_gc_collect_internal();
    }
}

static void *allocate_small_stress(size_t size, bool pretenured) {
    lock_heap();
    ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    if (block != NULL && !pretenured) {
        block->generation = SCOOP_GC_YOUNG;
        scoop_gc_heap_state.nursery_bytes += GC_BLOCK_SIZE;
    }
    unlock_heap();
    if (block == NULL) {
        heap_fatal("stress heap exhausted by permanently quarantined blocks");
    }
    void *object = (char *)block_base(block) + GC_LINE_SIZE;
    if ((char *)object + size > (char *)block_base(block) + GC_BLOCK_SIZE) {
        heap_fatal("stress small allocation exceeds its block");
    }
    return object;
}

static void *allocate_large(size_t size, ScoopGcBlockMeta **allocated_block) {
    bool collected = false;
    for (;;) {
        lock_heap();
        size_t span = scoop_heap_large_mapping_size(size);
        bool threshold_exceeded =
            !collected && span > collection_threshold - (committed_bytes < collection_threshold
                                                             ? committed_bytes
                                                             : collection_threshold);
        ScoopGcBlockMeta *block =
            threshold_exceeded ? NULL : activate_large_block(size, SCOOP_BLOCK_MUTATOR);
        unlock_heap();
        if (block != NULL) {
            *allocated_block = block;
            return (char *)block_base(block) + GC_LINE_SIZE;
        }
        if (collected) {
            heap_fatal("GC heap exhausted allocating a large object");
        }
        collected = scoop_gc_collect_internal();
    }
}

static void *allocate_large_stress(size_t size, ScoopGcBlockMeta **allocated_block) {
    lock_heap();
    ScoopGcBlockMeta *block = activate_large_block(size, SCOOP_BLOCK_MUTATOR);
    unlock_heap();
    if (block == NULL) {
        heap_fatal("stress heap exhausted by permanently quarantined blocks");
    }
    *allocated_block = block;
    return (char *)block_base(block) + GC_LINE_SIZE;
}

static void record_allocation(ScoopThreadState *thread, size_t size, bool nursery) {
    ScoopAllocationCounters *counters = &thread->allocation_counters;
    atomic_store_explicit(&counters->objects,
                          atomic_load_explicit(&counters->objects, memory_order_relaxed) + 1,
                          memory_order_relaxed);
    atomic_store_explicit(&counters->bytes,
                          atomic_load_explicit(&counters->bytes, memory_order_relaxed) + size,
                          memory_order_relaxed);
    if (nursery) {
        uint64_t objects = atomic_load_explicit(&counters->nursery_objects, memory_order_relaxed);
        uint64_t bytes = atomic_load_explicit(&counters->nursery_bytes, memory_order_relaxed);
        atomic_store_explicit(&counters->nursery_objects, objects + 1, memory_order_relaxed);
        atomic_store_explicit(&counters->nursery_bytes, bytes + size, memory_order_relaxed);
    }
}

static void finish_small_allocation(ScoopThreadState *thread, void *object,
                                    const ScoopTypeDescriptor *td, size_t size) {
    ScoopGcBlockMeta *block = thread->allocation_block;
    if (block == NULL || (uintptr_t)object < (uintptr_t)block_base(block) ||
        (uintptr_t)object - (uintptr_t)block_base(block) >= GC_BLOCK_SIZE) {
        block = pointer_block(object);
    }
    if (object == NULL || td == NULL || size > GC_REGULAR_MAX ||
        (uintptr_t)object % td->instance_shape.instance_alignment != 0 || block == NULL) {
        heap_fatal("invalid inline TLAB allocation");
    }
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    record_small_object(block, object, size, false);
    record_allocation(thread, size, block->generation == SCOOP_GC_YOUNG);
}

void scoop_runtime_finish_tlab_alloc(void *object, const ScoopTypeDescriptor *td, size_t size) {
    if (scoop_gc_stress_move_enabled() || td->release_hook != NULL) {
        heap_fatal("inline TLAB allocation violates its nursery contract");
    }
    finish_small_allocation(scoop_thread_current_required(), object, td,
                            scoop_shape_normalize_allocation(td, size));
}

void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size) {
    if (td == NULL) {
        heap_fatal("allocation has no TypeDescriptor");
    }
    ScoopThreadState *thread = scoop_thread_current_required();
    size = scoop_shape_normalize_allocation(td, size);
    bool stress = scoop_gc_stress_move_enabled();
    if (stress) {
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        thread->allocation_block = NULL;
        scoop_gc_collect_internal();
        if (thread->allocation.cursor != NULL || thread->allocation.limit != NULL) {
            heap_fatal("stress collection published a mutator TLAB");
        }
    }
    if (size <= GC_REGULAR_MAX) {
        void *object =
            stress ? allocate_small_stress(size, td->release_hook != NULL)
            : td->release_hook != NULL
                ? allocate_old(size, (size_t)td->instance_shape.instance_alignment)
                : allocate_small(thread, size, (size_t)td->instance_shape.instance_alignment);
        finish_small_allocation(thread, object, td, size);
        return object;
    }
    ScoopGcBlockMeta *block;
    void *object = stress ? allocate_large_stress(size, &block) : allocate_large(size, &block);
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    publish_large_object(block, false);
    record_allocation(thread, size, false);
    return object;
}
