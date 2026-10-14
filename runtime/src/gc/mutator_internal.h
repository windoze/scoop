#ifndef SCOOP_GC_MUTATOR_INTERNAL_H
#define SCOOP_GC_MUTATOR_INTERNAL_H

#include "../generated_entries.h"
#include "mutator.h"

typedef struct ScoopGcAllocationCounters {
    _Atomic(uint64_t) objects;
    _Atomic(uint64_t) nursery_objects;
    _Atomic(uint64_t) bytes;
    _Atomic(uint64_t) nursery_bytes;
} ScoopGcAllocationCounters;

struct ScoopGcMutator {
    ScoopAllocationContext allocation;
    struct ScoopGcBlockMeta *allocation_block;
    _Alignas(128) ScoopGcAllocationCounters counters;
};

_Static_assert(offsetof(ScoopGcMutator, allocation) == 0,
               "the generated TLAB context must be the mutator prefix");
_Static_assert(offsetof(ScoopGcMutator, counters) % 128 == 0 && _Alignof(ScoopGcMutator) == 128,
               "allocation counters must occupy a separate cache line");

_Noreturn void scoop_gc_mutator_fatal(const char *message);

static inline ScoopGcMutator *scoop_gc_current_mutator(void) {
    ScoopAllocationContext *context = scoop_rt_allocation_context;
    if (context == NULL) {
        scoop_gc_mutator_fatal("unattached thread entered the Scoop allocator");
    }
    return (ScoopGcMutator *)context;
}

static inline void scoop_gc_reset_allocation(ScoopGcMutator *mutator) {
    mutator->allocation.cursor = NULL;
    mutator->allocation.limit = NULL;
    mutator->allocation_block = NULL;
}

#endif
