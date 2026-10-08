#include "internal.h"

void scoop_thread_park_current_locked(ScoopThreadState *state) {
    ScoopThreadMode from = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (from != SCOOP_THREAD_MANAGED && from != SCOOP_THREAD_MANAGED_PENDING &&
        from != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_thread_fatal("only managed or native-borrowed threads may park");
    }
    if ((from == SCOOP_THREAD_MANAGED) != (state->managed_anchor != NULL)) {
        scoop_thread_fatal("parked thread has an invalid managed anchor");
    }
    if (from == SCOOP_THREAD_NATIVE_BORROWED &&
        (state->current_transition == NULL || state->managed_stack_boundary != NULL)) {
        scoop_thread_fatal("native-borrowed thread has no active transition");
    }

    /* These non-atomic fields are the collector's immutable snapshot for the
     * whole parked interval. A condition-variable wait may wake spuriously
     * while the collector scans outside scoop_thread_world_lock, so never
     * rewrite the anchor/root chains in the acknowledgement loop below. */
    state->parked_from = from;
    while (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
           SCOOP_WORLD_RUNNING) {
        uint64_t epoch = atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire);
        atomic_store_explicit(&state->observed_gc_epoch, epoch, memory_order_release);
        atomic_store_explicit(&state->mode, SCOOP_THREAD_PARKED, memory_order_release);
        scoop_thread_world_broadcast();
        /* Wait once rather than hiding phase changes in
         * scoop_thread_wait_for_running_world: a new collector may win the
         * world lock before an old-epoch parker wakes. The outer loop must then
         * acknowledge the new epoch while the same spill/SP are still valid. */
        scoop_thread_world_wait();
    }

    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, from, memory_order_release);
}

void scoop_thread_poll(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (state->managed_anchor == NULL) {
        scoop_thread_fatal("managed poll has no published anchor");
    }
    uint64_t epoch = atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire);
    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) ==
            SCOOP_WORLD_RUNNING &&
        atomic_load_explicit(&state->observed_gc_epoch, memory_order_acquire) == epoch) {
        return;
    }

    scoop_thread_registry_lock();
    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    } else {
        atomic_store_explicit(&state->observed_gc_epoch,
                              atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
                              memory_order_release);
    }
    scoop_thread_registry_unlock();
}

void scoop_thread_native_borrowed_entry(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (mode != SCOOP_THREAD_NATIVE_BORROWED) {
        scoop_thread_fatal("native-borrowed runtime entry from an invalid mode");
    }
    if (state->managed_anchor != NULL || state->managed_stack_boundary != NULL ||
        state->current_transition == NULL || state->caller_roots == NULL ||
        state->current_transition->caller_roots != state->caller_roots ||
        state->current_transition->managed_return_pc == 0 ||
        state->current_transition->managed_stack_pointer == 0 ||
        state->current_transition->managed_frame_pointer == 0 ||
        state->current_transition->managed_stack_high == 0) {
        scoop_thread_fatal("native-borrowed runtime entry has incomplete published roots");
    }

    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        scoop_thread_registry_lock();
        scoop_thread_park_current_locked(state);
        scoop_thread_registry_unlock();
    }
}

void scoop_thread_push_managed_anchor(ScoopManagedAnchor *anchor, uintptr_t return_pc,
                                      uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    uintptr_t boundary = (uintptr_t)state->managed_stack_boundary;
    scoop_thread_ensure_stack_range(state, stack_pointer, boundary);
    uintptr_t stack_low = (uintptr_t)state->stack_low;
    uintptr_t stack_high = (uintptr_t)state->stack_high;
    if (anchor == NULL || return_pc == 0 || stack_pointer < stack_low ||
        stack_pointer >= boundary || frame_pointer < stack_pointer || frame_pointer >= boundary ||
        boundary > stack_high || state->managed_anchor != NULL) {
        scoop_thread_fatal("managed entry published an invalid anchor");
    }
    *anchor = (ScoopManagedAnchor){
        .return_pc = return_pc,
        .stack_pointer = stack_pointer,
        .frame_pointer = frame_pointer,
        .previous = NULL,
    };
    state->managed_anchor = anchor;
}

void scoop_thread_push_safepoint_anchor(ScoopManagedAnchor *anchor, uintptr_t return_pc,
                                        uintptr_t stack_pointer, uintptr_t frame_pointer) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != SCOOP_THREAD_MANAGED_PENDING) {
        scoop_thread_push_managed_anchor(anchor, return_pc, stack_pointer, frame_pointer);
        return;
    }
    /* Only the compiler's first poll may activate an argument-free gateway.
     * The collector can scan outer roots while this new segment is empty. */
    scoop_thread_registry_lock();
    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    }
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    scoop_thread_push_managed_anchor(anchor, return_pc, stack_pointer, frame_pointer);
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
                          memory_order_release);
    scoop_thread_registry_unlock();
}

void scoop_thread_pop_managed_anchor(ScoopManagedAnchor *anchor) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (anchor == NULL || state->managed_anchor != anchor || anchor->previous != NULL) {
        scoop_thread_fatal("managed entry anchors must be popped in LIFO order");
    }
    state->managed_anchor = NULL;
    *anchor = (ScoopManagedAnchor){0};
}

static bool all_collection_targets_quiescent(ScoopThreadState *collector, uint64_t epoch,
                                             uint64_t *parked_count, uint64_t *native_safe_count) {
    uint64_t parked = 0;
    uint64_t native_safe = 0;
    for (ScoopThreadState *state = scoop_thread_registry; state != NULL;
         state = state->registry_next) {
        if (state == collector) {
            continue;
        }
        ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_seq_cst);
        if (mode == SCOOP_THREAD_NATIVE_SAFE) {
            native_safe++;
            continue;
        }
        if (mode == SCOOP_THREAD_MANAGED_PENDING) {
            continue;
        }
        if (mode == SCOOP_THREAD_PARKED &&
            atomic_load_explicit(&state->observed_gc_epoch, memory_order_acquire) == epoch) {
            parked++;
            continue;
        }
        if (mode == SCOOP_THREAD_MANAGED || mode == SCOOP_THREAD_NATIVE_BORROWED ||
            mode == SCOOP_THREAD_NATIVE_SAFE_RETURNING || mode == SCOOP_THREAD_PARKED) {
            return false;
        }
        scoop_thread_fatal("invalid thread mode while stopping the world");
    }
    *parked_count = parked;
    *native_safe_count = native_safe;
    return true;
}

bool scoop_thread_begin_collection(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode requester_mode = atomic_load_explicit(&state->mode, memory_order_acquire);
    if ((requester_mode != SCOOP_THREAD_MANAGED &&
         requester_mode != SCOOP_THREAD_NATIVE_BORROWED) ||
        (requester_mode == SCOOP_THREAD_MANAGED && state->managed_depth == 0)) {
        scoop_thread_fatal("collection requested from an invalid thread mode");
    }
    if ((requester_mode == SCOOP_THREAD_MANAGED && state->managed_anchor == NULL) ||
        (requester_mode == SCOOP_THREAD_NATIVE_BORROWED &&
         (state->managed_anchor != NULL || state->current_transition == NULL))) {
        scoop_thread_fatal("collection requester has no exact root publication");
    }

    scoop_thread_registry_lock();
    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
        scoop_thread_registry_unlock();
        return false;
    }
    /* The RUNNING -> STOPPING transition under the world lock selects exactly
     * one collector; later requests join its epoch instead of acquiring a
     * second mutex in the opposite order at collection end. */
    uint64_t epoch = atomic_fetch_add_explicit(&scoop_thread_gc_epoch, 1, memory_order_acq_rel) + 1;
    atomic_store_explicit(&scoop_thread_world_phase, SCOOP_WORLD_STOPPING, memory_order_seq_cst);
    SCOOP_THREAD_TEST_POINT(SCOOP_TEST_COLLECTOR_STOPPING);
    state->parked_from = requester_mode;
    atomic_store_explicit(&state->observed_gc_epoch, epoch, memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_COLLECTOR, memory_order_release);
    scoop_thread_world_broadcast();

    uint64_t parked_count = 0;
    uint64_t native_safe_count = 0;
    while (!all_collection_targets_quiescent(state, epoch, &parked_count, &native_safe_count)) {
        SCOOP_THREAD_TEST_POINT(SCOOP_TEST_COLLECTOR_WAITING);
        scoop_thread_world_wait_for_quiescence();
    }
    atomic_store_explicit(&scoop_thread_last_gc_parked_count, parked_count, memory_order_release);
    atomic_store_explicit(&scoop_thread_last_gc_native_safe_count, native_safe_count,
                          memory_order_release);
    atomic_store_explicit(&scoop_thread_world_phase, SCOOP_WORLD_COLLECTING, memory_order_release);
    SCOOP_THREAD_TEST_POINT(SCOOP_TEST_COLLECTOR_STOPPED);
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
    return true;
}

void scoop_thread_end_collection(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != SCOOP_THREAD_COLLECTOR) {
        scoop_thread_fatal("collection ended by a non-collector thread");
    }

    scoop_thread_registry_lock();
    if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
        SCOOP_WORLD_COLLECTING) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("collection ended outside the collecting phase");
    }
    ScoopThreadMode requester_mode = state->parked_from;
    atomic_store_explicit(&state->mode, requester_mode, memory_order_release);
    SCOOP_THREAD_TEST_POINT(SCOOP_TEST_COLLECTOR_RESUMING);
    atomic_store_explicit(&scoop_thread_world_phase, SCOOP_WORLD_RUNNING, memory_order_release);
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
}

ScoopThreadState *scoop_thread_collection_registry_head(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != SCOOP_THREAD_COLLECTOR ||
        atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
            SCOOP_WORLD_COLLECTING) {
        scoop_thread_fatal("thread registry enumerated outside STW collection");
    }
    return scoop_thread_registry;
}
