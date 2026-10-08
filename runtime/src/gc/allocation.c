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

bool scoop_heap_take_free_run(size_t size, char **cursor, char **limit) {
    ScoopGcFreeRun **available = &free_runs;
    while (*available != NULL && (size_t)(*available)->line_count * GC_LINE_SIZE < size) {
        available = &(*available)->next;
    }
    ScoopGcFreeRun *run = *available;
    if (run == NULL) {
        return false;
    }
    *available = run->next;
    *cursor = (char *)block_base(run->block_index) + (size_t)run->first_line * GC_LINE_SIZE;
    *limit = *cursor + (size_t)run->line_count * GC_LINE_SIZE;
    free(run);
    return true;
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
    uint32_t index = activate_small_block(SCOOP_BLOCK_MUTATOR);
    if (index == UINT32_MAX) {
        unlock_heap();
        return REFILL_FULL;
    }
    blocks[index].generation = SCOOP_GC_YOUNG;
    scoop_gc_heap_state.nursery_bytes += GC_BLOCK_SIZE;
    thread->allocation.cursor = (char *)block_base(index) + GC_LINE_SIZE;
    thread->allocation.limit = (char *)block_base(index) + GC_BLOCK_SIZE;
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
        RefillResult refill = refill_nursery(thread, full_attempted);
        if (refill == REFILL_READY) {
            continue;
        }
        if (refill == REFILL_MINOR) {
            /* Another mutator can claim the newly emptied nursery before
             * this thread resumes. Capacity contention is not arena OOM. */
            scoop_gc_collect_minor_internal();
            full_attempted = false;
        } else if (!full_attempted) {
            full_attempted = scoop_gc_collect_internal();
        } else {
            heap_fatal("GC arena exhausted allocating a regular object");
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
            uint32_t index = activate_small_block(SCOOP_BLOCK_MUTATOR);
            if (index != UINT32_MAX) {
                *cursor = (char *)block_base(index) + GC_LINE_SIZE;
                *limit = (char *)block_base(index) + GC_BLOCK_SIZE;
                object = scoop_heap_bump(cursor, *limit, size, alignment);
            }
        }
        unlock_heap();
        if (object != NULL) {
            return object;
        }
        if (collected) {
            heap_fatal("GC arena exhausted allocating a pretenured object");
        }
        collected = scoop_gc_collect_internal();
    }
}

static void *allocate_small_stress(size_t size, bool pretenured) {
    lock_heap();
    uint32_t index = activate_small_block(SCOOP_BLOCK_MUTATOR);
    if (index != UINT32_MAX && !pretenured) {
        blocks[index].generation = SCOOP_GC_YOUNG;
        scoop_gc_heap_state.nursery_bytes += GC_BLOCK_SIZE;
    }
    unlock_heap();
    if (index == UINT32_MAX) {
        heap_fatal("stress arena exhausted by permanently quarantined blocks");
    }
    void *object = (char *)block_base(index) + GC_LINE_SIZE;
    if ((char *)object + size > (char *)block_base(index) + GC_BLOCK_SIZE) {
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
        size_t span = (GC_LINE_SIZE + size + GC_BLOCK_SIZE - 1) & ~(GC_BLOCK_SIZE - 1);
        bool threshold_exceeded = !collected && committed_bytes + span > collection_threshold;
        uint32_t index =
            threshold_exceeded ? UINT32_MAX : activate_large_block(size, SCOOP_BLOCK_MUTATOR);
        unlock_heap();
        if (index != UINT32_MAX) {
            *block_index = index;
            return (char *)block_base(index) + GC_LINE_SIZE;
        }
        if (collected) {
            heap_fatal("GC arena exhausted allocating a large object");
        }
        collected = scoop_gc_collect_internal();
    }
}

static void *allocate_large_stress(size_t size, uint32_t *block_index) {
    lock_heap();
    uint32_t index = activate_large_block(size, SCOOP_BLOCK_MUTATOR);
    unlock_heap();
    if (index == UINT32_MAX) {
        heap_fatal("stress arena exhausted by permanently quarantined blocks");
    }
    *block_index = index;
    return (char *)block_base(index) + GC_LINE_SIZE;
}

static void finish_small_allocation(void *object, const ScoopTypeDescriptor *td, size_t size) {
    uint32_t block_index;
    if (object == NULL || td == NULL || size > GC_REGULAR_MAX ||
        (uintptr_t)object % td->instance_shape.instance_alignment != 0 ||
        !pointer_block_index(object, &block_index)) {
        heap_fatal("invalid inline TLAB allocation");
    }
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    record_small_object(block_index, object, size, false);
    atomic_fetch_add_explicit(&live_objects, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&scoop_gc_heap_state.allocated_bytes, size, memory_order_relaxed);
    if (blocks[block_index].generation == SCOOP_GC_YOUNG) {
        atomic_fetch_add_explicit(&scoop_gc_heap_state.nursery_objects, 1, memory_order_relaxed);
        atomic_fetch_add_explicit(&scoop_gc_heap_state.nursery_allocated_bytes, size,
                                  memory_order_relaxed);
    }
}

void scoop_runtime_finish_tlab_alloc(void *object, const ScoopTypeDescriptor *td, size_t size) {
    if (scoop_gc_stress_move_enabled() || td->release_hook != NULL) {
        heap_fatal("inline TLAB allocation violates its nursery contract");
    }
    finish_small_allocation(object, td, scoop_shape_normalize_allocation(td, size));
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
        finish_small_allocation(object, td, size);
        return object;
    }
    uint32_t block_index;
    void *object =
        stress ? allocate_large_stress(size, &block_index) : allocate_large(size, &block_index);
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    header->td = td;
    header->gc_word = 0;
    publish_large_object(block_index, false);
    atomic_fetch_add_explicit(&live_objects, 1, memory_order_relaxed);
    atomic_fetch_add_explicit(&scoop_gc_heap_state.allocated_bytes, size, memory_order_relaxed);
    return object;
}
