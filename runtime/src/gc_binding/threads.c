/* Adapt the existing world/registry lifetime without a second thread directory. */
#include "../thread/internal.h"
#include "../gc_binding.h"

bool scoop_gc_stop_world(void) { return scoop_thread_begin_collection(); }
void scoop_gc_resume_world(void) { scoop_thread_end_collection(); }
void scoop_gc_lock_mutators(void) { scoop_thread_registry_lock(); }
void scoop_gc_unlock_mutators(void) { scoop_thread_registry_unlock(); }

void scoop_gc_retire_allocations(void) {
    for (ScoopThreadState *thread = scoop_thread_collection_registry_head(); thread != NULL;
         thread = thread->registry_next) {
        scoop_gc_mutator_invalidate(thread->mutator);
    }
}

ScoopGcAllocationTotals scoop_gc_allocation_totals_locked(void) {
    ScoopGcAllocationTotals total = scoop_gc_detached_allocations_locked();
    for (ScoopThreadState *thread = scoop_thread_registry; thread != NULL;
         thread = thread->registry_next) {
        scoop_gc_add_mutator_totals(thread->mutator, &total);
    }
    return total;
}

void scoop_gc_resident_memory(uint64_t *current, uint64_t *peak) {
    scoop_platform_bundle()->thread_vm->resident_memory(current, peak);
}

#ifdef SCOOP_THREAD_TESTING
void scoop_gc_nursery_retry_test_point(void) { SCOOP_THREAD_TEST_POINT(SCOOP_TEST_NURSERY_RETRY); }
#endif
