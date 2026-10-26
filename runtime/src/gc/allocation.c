/* Mutator allocation paths: owner-only TLAB bump/refill and large objects. */
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "mutator_internal.h"
#include "../gc_binding/object.h"
#include "gc_internal.h"
#include "heap_internal.h"
#include "allocation_metadata.h"

_Static_assert(GC_BLOCK_SIZE == 32768 && GC_LINE_SIZE == 128 && sizeof(uint64_t) == 8,
               "generated TLAB metadata geometry drifted");
_Static_assert(GC_LINE_SIZE % SCOOP_MAXIMUM_MANAGED_ALIGNMENT == 0,
               "GC line starts must satisfy every managed alignment");
_Static_assert(GC_REGULAR_MAX / sizeof(uint64_t) <= UINT16_MAX,
               "regular object sizes must fit exact side metadata");

void *scoop_heap_bump(char **cursor, char *limit, size_t size, size_t alignment) {
    if (*cursor == NULL) {
        return NULL;
    }
    char *object = (char *)scoop_gc_align_address((uintptr_t)*cursor, alignment);
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

static bool exceeds_old_threshold(size_t bytes) {
    size_t occupied = committed_bytes;
    if (scoop_heap_adaptive_nursery_enabled()) {
        occupied -= scoop_gc_heap_state.nursery_bytes;
    }
    return occupied > collection_threshold || bytes > collection_threshold - occupied;
}

static void record_nursery_block(ScoopGcBlockMeta *block) {
    block->generation = SCOOP_GC_YOUNG;
    block->next_collection = scoop_gc_heap_state.young_blocks;
    scoop_gc_heap_state.young_blocks = block;
    scoop_gc_heap_state.nursery_bytes += GC_BLOCK_SIZE;
}

static void publish_tlab(ScoopGcMutator *mutator, ScoopGcBlockMeta *block) {
    mutator->allocation_block = block;
    ScoopAllocationContext *allocation = &mutator->allocation;
    allocation->block_start = block_base(block);
    allocation->size_units = block->size_units;
    allocation->starts = block->starts;
    allocation->line_occupied = block->line_occupied;
    allocation->limit = allocation->block_start + GC_BLOCK_SIZE;
    allocation->cursor = allocation->block_start + GC_LINE_SIZE;
}

static RefillResult refill_nursery(ScoopGcMutator *mutator, bool after_full) {
    if (mutator->reserved_count != 0) {
        publish_tlab(mutator, mutator->reserved_blocks[--mutator->reserved_count]);
        return REFILL_READY;
    }
    lock_heap();
    size_t capacity = scoop_gc_heap_state.nursery_capacity;
#if SCOOP_ENABLE_GC_STRESS
    if (scoop_gc_heap_state.stress_minor) {
        capacity = GC_BLOCK_SIZE;
    }
#endif
    size_t old_request = scoop_heap_adaptive_nursery_enabled() ? 0 : GC_BLOCK_SIZE;
    if (!after_full && exceeds_old_threshold(old_request)) {
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
    record_nursery_block(block);
    if (scoop_heap_adaptive_nursery_enabled()) {
        while (mutator->reserved_count < 3 &&
               scoop_gc_heap_state.nursery_bytes < capacity) {
            ScoopGcBlockMeta *reserve = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
            if (reserve == NULL) {
                break;
            }
            record_nursery_block(reserve);
            mutator->reserved_blocks[mutator->reserved_count++] = reserve;
        }
    }
    unlock_heap();
    publish_tlab(mutator, block);
    return REFILL_READY;
}

static void *allocate_small(ScoopGcMutator *mutator, size_t size, size_t alignment) {
    bool full_attempted = false;
    for (;;) {
        void *object = scoop_heap_bump(&mutator->allocation.cursor, mutator->allocation.limit, size,
                                       alignment);
        if (object != NULL) {
            return object;
        }
        scoop_gc_reset_current_allocation(mutator);
        RefillResult refill = refill_nursery(mutator, full_attempted);
        if (refill == REFILL_READY) {
            continue;
        }
        if (refill == REFILL_MINOR) {
            /* Another mutator can claim the newly emptied nursery before
             * this mutator resumes. Capacity contention is not heap OOM. */
            scoop_gc_collect_minor_internal();
            full_attempted = false;
        } else if (!full_attempted) {
            full_attempted = scoop_gc_collect_internal();
        } else {
            heap_fatal("GC heap exhausted allocating a regular object");
        }
        scoop_gc_nursery_retry_test_point();
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
        if (object == NULL && (collected || !exceeds_old_threshold(GC_BLOCK_SIZE))) {
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

#if SCOOP_ENABLE_GC_STRESS
static void *allocate_small_stress(size_t size, bool pretenured) {
    lock_heap();
    ScoopGcBlockMeta *block = activate_small_block(SCOOP_BLOCK_MUTATOR, true);
    if (block != NULL && !pretenured) {
        record_nursery_block(block);
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
#endif

static void *allocate_large(size_t size, ScoopGcBlockMeta **allocated_block) {
    bool collected = false;
    for (;;) {
        lock_heap();
        size_t span = scoop_heap_large_mapping_size(size);
        bool threshold_exceeded = !collected && exceeds_old_threshold(span);
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

#if SCOOP_ENABLE_GC_STRESS
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
#endif

static void record_allocation(ScoopGcMutator *mutator, size_t size, bool nursery) {
    ScoopGcAllocationCounters *counters = &mutator->counters;
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

static void initialize_small_allocation(ScoopGcMutator *mutator, ScoopGcBlockMeta *block,
                                        void *object, const ScoopTypeDescriptor *td, size_t size,
                                        bool nursery) {
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    scoop_gc_set_object_type(object, td);
    header->gc_word = 0;
    if (nursery) {
        scoop_heap_record_small_metadata(block, object, size, false, true);
    } else {
        record_small_object(block, object, size);
    }
    record_allocation(mutator, size, nursery);
}

static void finish_small_allocation(ScoopGcMutator *mutator, void *object,
                                    const ScoopTypeDescriptor *td, size_t size) {
    ScoopGcBlockMeta *block = mutator->allocation_block;
    if (block == NULL || (uintptr_t)object < (uintptr_t)block_base(block) ||
        (uintptr_t)object - (uintptr_t)block_base(block) >= GC_BLOCK_SIZE) {
        block = pointer_block(object);
    }
    if (object == NULL || td == NULL || size > GC_REGULAR_MAX ||
        (uintptr_t)object % scoop_gc_type_alignment(td) != 0 || block == NULL) {
        heap_fatal("invalid inline TLAB allocation");
    }
    initialize_small_allocation(mutator, block, object, td, size,
                                block->generation == SCOOP_GC_YOUNG);
}

void scoop_runtime_finish_tlab_alloc(void *object, const ScoopTypeDescriptor *td, size_t size) {
    /* Generated code has already aligned and bounded this nursery bump. */
    ScoopGcMutator *mutator = scoop_gc_current_mutator();
    initialize_small_allocation(mutator, mutator->allocation_block, object, td, size, true);
}

void *scoop_gc_alloc_internal(const ScoopTypeDescriptor *td, size_t size) {
    if (td == NULL) {
        heap_fatal("allocation has no TypeDescriptor");
    }
    ScoopGcMutator *mutator = scoop_gc_current_mutator();
    size = scoop_gc_allocation_size(td, size);
#if SCOOP_ENABLE_GC_STRESS
    bool stress = scoop_gc_stress_move_enabled();
    if (stress) {
        scoop_gc_reset_allocation(mutator);
        scoop_gc_collect_internal();
        if (mutator->allocation.cursor != NULL || mutator->allocation.limit != NULL) {
            heap_fatal("stress collection published a mutator TLAB");
        }
    }
#endif
    if (size <= GC_REGULAR_MAX) {
        void *object =
#if SCOOP_ENABLE_GC_STRESS
            stress ? allocate_small_stress(size, scoop_gc_type_has_release(td)) :
#endif
                         scoop_gc_type_has_release(td)
                           ? allocate_old(size, scoop_gc_type_alignment(td))
                           : allocate_small(mutator, size, scoop_gc_type_alignment(td));
        finish_small_allocation(mutator, object, td, size);
        return object;
    }
    ScoopGcBlockMeta *block;
    void *object =
#if SCOOP_ENABLE_GC_STRESS
        stress ? allocate_large_stress(size, &block) :
#endif
                 allocate_large(size, &block);
    memset(object, 0, size);
    ScoopObjectHeader *header = object;
    scoop_gc_set_object_type(object, td);
    header->gc_word = 0;
    publish_large_object(block, false);
    record_allocation(mutator, size, false);
    return object;
}
