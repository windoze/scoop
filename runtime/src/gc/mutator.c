#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "mutator_internal.h"

_Thread_local ScoopAllocationContext *scoop_rt_allocation_context;
static ScoopGcAllocationTotals detached_allocations;

_Noreturn void scoop_gc_mutator_fatal(const char *message) {
    fprintf(stderr, "scoop gc: %s\n", message);
    abort();
}

ScoopGcMutator *scoop_gc_mutator_create(void) {
    void *storage = NULL;
    if (posix_memalign(&storage, _Alignof(ScoopGcMutator), sizeof(ScoopGcMutator)) != 0) {
        scoop_gc_mutator_fatal("out of memory creating GC mutator state");
    }
    ScoopGcMutator *mutator = storage;
    memset(mutator, 0, sizeof *mutator);
    atomic_init(&mutator->counters.objects, 0);
    atomic_init(&mutator->counters.nursery_objects, 0);
    atomic_init(&mutator->counters.bytes, 0);
    atomic_init(&mutator->counters.nursery_bytes, 0);
    mutator->allocation.counters = &mutator->counters;
    return mutator;
}

void scoop_gc_mutator_bind(ScoopGcMutator *mutator) {
    scoop_rt_allocation_context = &mutator->allocation;
}

void scoop_gc_mutator_invalidate(ScoopGcMutator *mutator) { scoop_gc_reset_allocation(mutator); }

bool scoop_gc_mutator_is_retired(const ScoopGcMutator *mutator) {
    return mutator->allocation.cursor == NULL && mutator->allocation.limit == NULL &&
           mutator->allocation_block == NULL;
}

void scoop_gc_add_mutator_totals(const ScoopGcMutator *mutator, ScoopGcAllocationTotals *total) {
    const ScoopGcAllocationCounters *counters = &mutator->counters;
    total->objects += atomic_load_explicit(&counters->objects, memory_order_relaxed);
    total->nursery_objects +=
        atomic_load_explicit(&counters->nursery_objects, memory_order_relaxed);
    total->bytes += atomic_load_explicit(&counters->bytes, memory_order_relaxed);
    total->nursery_bytes += atomic_load_explicit(&counters->nursery_bytes, memory_order_relaxed);
}

ScoopGcAllocationTotals scoop_gc_detached_allocations_locked(void) { return detached_allocations; }

void scoop_gc_mutator_detach(ScoopGcMutator *mutator) {
    scoop_gc_add_mutator_totals(mutator, &detached_allocations);
    scoop_rt_allocation_context = NULL;
    free(mutator);
}
