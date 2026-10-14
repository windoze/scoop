#ifndef SCOOP_GC_MUTATOR_H
#define SCOOP_GC_MUTATOR_H

#include <stdbool.h>
#include <stdint.h>

typedef struct ScoopGcMutator ScoopGcMutator;

typedef struct ScoopGcAllocationTotals {
    uint64_t objects;
    uint64_t nursery_objects;
    uint64_t bytes;
    uint64_t nursery_bytes;
} ScoopGcAllocationTotals;

ScoopGcMutator *scoop_gc_mutator_create(void);
void scoop_gc_mutator_bind(ScoopGcMutator *mutator);
/* Owner or STW coordinator only; retire the complete allocation range. */
void scoop_gc_mutator_invalidate(ScoopGcMutator *mutator);
bool scoop_gc_mutator_is_retired(const ScoopGcMutator *mutator);
/* Registry/world lock protects detach and cumulative accounting. */
void scoop_gc_mutator_detach(ScoopGcMutator *mutator);
ScoopGcAllocationTotals scoop_gc_detached_allocations_locked(void);
void scoop_gc_add_mutator_totals(const ScoopGcMutator *mutator, ScoopGcAllocationTotals *total);

#endif
