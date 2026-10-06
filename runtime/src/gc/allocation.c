/* Mutator allocation paths: owner-only TLAB bump/refill and large objects. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "../thread.h"
#include "../value_shape.h"
#include "gc_internal.h"
#include "heap_internal.h"

_Static_assert(GC_LINE_SIZE % SCOOP_MAXIMUM_MANAGED_ALIGNMENT == 0,
               "GC line starts must satisfy every managed alignment");
_Static_assert(GC_REGULAR_MAX / sizeof(uint64_t) <= UINT16_MAX,
               "regular object sizes must fit exact side metadata");

static void *tlab_allocate(ScoopThreadState *thread, size_t size, size_t alignment) {
    char *cursor = thread->allocation.cursor;
    if (cursor == NULL) {
        return NULL;
    }
    cursor = (char *)scoop_shape_align((uintptr_t)cursor, alignment);
    if (cursor > thread->allocation.limit ||
        size > (size_t)(thread->allocation.limit - cursor)) {
        return NULL;
    }
    thread->allocation.cursor = cursor + size;
    return cursor;
}

static bool refill_tlab(ScoopThreadState *thread, size_t size) {
    lock_heap();
    ScoopGcFreeRun **available = &free_runs;
    while (*available != NULL &&
           (size_t)(*available)->line_count * GC_LINE_SIZE < size) {
        available = &(*available)->next;
    }
    ScoopGcFreeRun *run = *available;
    if (run != NULL) {
        *available = run->next;
        thread->allocation.cursor = (char *)block_base(run->block_index) +
                                    (size_t)run->first_line * GC_LINE_SIZE;
        thread->allocation.limit =
            thread->allocation.cursor + (size_t)run->line_count * GC_LINE_SIZE;
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
        thread->allocation.cursor = (char *)block_base(index) + GC_LINE_SIZE;
        thread->allocation.limit = (char *)block_base(index) + GC_BLOCK_SIZE;
    }
    unlock_heap();
    return index != UINT32_MAX;
}

static void *allocate_small(ScoopThreadState *thread, size_t size, size_t alignment) {
    bool collected = false;
    for (;;) {
        void *object = tlab_allocate(thread, size, alignment);
        if (object != NULL) {
            return object;
        }
        thread->allocation.cursor = NULL;
        thread->allocation.limit = NULL;
        if (refill_tlab(thread, size)) {
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
        bool threshold_exceeded =
            !collected && committed_bytes + span > collection_threshold;
        uint32_t index = threshold_exceeded
                             ? UINT32_MAX
                             : activate_large_block(size, SCOOP_BLOCK_MUTATOR);
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
    uint32_t index = activate_large_block(size, SCOOP_BLOCK_MUTATOR);
    unlock_heap();
    if (index == UINT32_MAX) {
        heap_fatal("stress arena exhausted by permanently quarantined blocks");
    }
    *block_index = index;
    return (char *)block_base(index) + GC_LINE_SIZE;
}

static void finish_small_allocation(void *object, const ScoopTypeDescriptor *td,
                                    size_t size) {
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
}

void scoop_runtime_finish_tlab_alloc(void *object, const ScoopTypeDescriptor *td,
                                     size_t size) {
    if (scoop_gc_stress_move_enabled()) {
        heap_fatal("inline TLAB allocation remained enabled in stress mode");
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
            stress ? allocate_small_stress(size)
                   : allocate_small(thread, size,
                                    (size_t)td->instance_shape.instance_alignment);
        finish_small_allocation(object, td, size);
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
