#include "internal.h"

static void enter_managed(const void *managed_stack_boundary,
                          ScoopManagedSegmentState segment) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_NATIVE_SAFE ||
        state->managed_depth != 0 ||
        state->managed_segment != SCOOP_MANAGED_SEGMENT_NONE) {
        scoop_thread_fatal("invalid managed entry transition");
    }
    uintptr_t boundary = (uintptr_t)managed_stack_boundary;
    if (boundary < (uintptr_t)state->stack_low ||
        boundary > (uintptr_t)state->stack_high) {
        scoop_thread_fatal("managed entry published an invalid stack boundary");
    }
    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    state->managed_depth = 1;
    state->managed_stack_boundary = managed_stack_boundary;
    state->managed_segment = segment;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    scoop_thread_registry_unlock();
}

void scoop_thread_enter_managed(const void *managed_stack_boundary) {
    enter_managed(managed_stack_boundary, SCOOP_MANAGED_SEGMENT_ACTIVE);
}

void scoop_thread_enter_gateway(const void *managed_stack_boundary) {
    enter_managed(managed_stack_boundary, SCOOP_MANAGED_SEGMENT_PENDING);
}
void scoop_thread_leave_managed(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (state->managed_depth != 1 || state->managed_anchor != NULL) {
        scoop_thread_fatal("invalid managed exit transition");
    }
    scoop_thread_registry_lock();
    state->managed_depth = 0;
    state->managed_stack_boundary = NULL;
    state->managed_segment = SCOOP_MANAGED_SEGMENT_NONE;
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_NATIVE_SAFE, memory_order_release);
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
}

void scoop_thread_enter_callback(ScoopCallbackThreadEntry *entry,
                                 const void *managed_stack_boundary) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode previous = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (entry == NULL || entry->active ||
        (previous != SCOOP_THREAD_NATIVE_SAFE &&
         previous != SCOOP_THREAD_NATIVE_BORROWED)) {
        scoop_thread_fatal("invalid managed callback entry transition");
    }
    uintptr_t boundary = (uintptr_t)managed_stack_boundary;
    if (boundary < (uintptr_t)state->stack_low ||
        boundary > (uintptr_t)state->stack_high) {
        scoop_thread_fatal("managed callback published an invalid stack boundary");
    }

    scoop_thread_registry_lock();
    if (previous == SCOOP_THREAD_NATIVE_SAFE) {
        scoop_thread_wait_for_running_world();
    } else if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
               SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    }
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != previous) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("native mode changed during managed callback entry");
    }
    entry->previous_mode = previous;
    entry->previous_managed_stack_boundary = state->managed_stack_boundary;
    entry->previous_managed_depth = state->managed_depth;
    entry->previous_managed_segment = state->managed_segment;
    entry->active = true;
    state->callback_depth++;
    state->managed_depth++;
    state->managed_stack_boundary = managed_stack_boundary;
    state->managed_segment = SCOOP_MANAGED_SEGMENT_ACTIVE;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    scoop_thread_registry_unlock();
}

void scoop_thread_leave_callback(ScoopCallbackThreadEntry *entry) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (entry == NULL || !entry->active || state->callback_depth == 0 ||
        state->managed_depth != entry->previous_managed_depth + 1 ||
        state->managed_anchor != NULL) {
        scoop_thread_fatal("invalid managed callback exit transition");
    }

    scoop_thread_registry_lock();
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    state->callback_depth--;
    state->managed_depth = entry->previous_managed_depth;
    state->managed_stack_boundary = entry->previous_managed_stack_boundary;
    state->managed_segment = entry->previous_managed_segment;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, entry->previous_mode, memory_order_release);
    entry->active = false;
    if (entry->previous_mode == SCOOP_THREAD_NATIVE_BORROWED &&
        atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
            SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    } else {
        scoop_thread_world_broadcast();
    }
    scoop_thread_registry_unlock();
}

static void enter_native(ScoopThreadTransition *transition, uintptr_t managed_stack_low,
                         uintptr_t return_pc, uintptr_t stack_pointer,
                         uintptr_t frame_pointer, ScoopThreadMode native_mode) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    uintptr_t managed_boundary = (uintptr_t)state->managed_stack_boundary;
    if (transition == NULL || state->managed_anchor != NULL || return_pc == 0 ||
        managed_stack_low < (uintptr_t)state->stack_low ||
        managed_stack_low >= managed_boundary ||
        stack_pointer < (uintptr_t)state->stack_low ||
        stack_pointer >= managed_boundary || frame_pointer < stack_pointer ||
        frame_pointer >= managed_boundary ||
        managed_boundary > (uintptr_t)state->stack_high) {
        scoop_thread_fatal(
            "native transition published an invalid managed stack segment");
    }
    for (ScoopThreadTransition *active = state->current_transition; active != NULL;
         active = active->previous) {
        if (active == transition) {
            scoop_thread_fatal("native transition record is already active");
        }
    }
    if (state->caller_roots == NULL) {
        scoop_thread_fatal("native transition requires a caller root frame");
    }
    if (state->current_transition != NULL &&
        state->current_transition->caller_roots == state->caller_roots) {
        scoop_thread_fatal("native transition requires a fresh caller root frame");
    }

    scoop_thread_registry_lock();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_MANAGED ||
        state->managed_depth == 0) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("native transition lost its managed source state");
    }
    transition->previous = state->current_transition;
    transition->caller_roots = state->caller_roots;
    transition->managed_return_pc = return_pc;
    transition->managed_stack_pointer = stack_pointer;
    transition->managed_frame_pointer = frame_pointer;
    transition->managed_stack_low = managed_stack_low;
    transition->managed_stack_high = managed_boundary;
    transition->previous_mode = SCOOP_THREAD_MANAGED;
    transition->native_mode = native_mode;
    state->current_transition = transition;
    state->managed_stack_boundary = NULL;
    state->managed_segment = SCOOP_MANAGED_SEGMENT_NONE;
    state->managed_depth--;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, native_mode, memory_order_release);
    if (native_mode == SCOOP_THREAD_NATIVE_BORROWED &&
        atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
            SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    } else {
        scoop_thread_world_broadcast();
    }
    scoop_thread_registry_unlock();
}

static void leave_native(ScoopThreadTransition *transition,
                         ScoopThreadMode expected_mode) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (transition == NULL || state->current_transition != transition ||
        atomic_load_explicit(&state->mode, memory_order_acquire) != expected_mode ||
        transition->native_mode != (uint32_t)expected_mode ||
        transition->previous_mode != (uint32_t)SCOOP_THREAD_MANAGED) {
        scoop_thread_fatal("native transitions must be left in LIFO order");
    }

    scoop_thread_registry_lock();
    if (expected_mode == SCOOP_THREAD_NATIVE_SAFE) {
        scoop_thread_wait_for_running_world();
    } else if (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
               SCOOP_WORLD_RUNNING) {
        scoop_thread_park_current_locked(state);
    }
    if (state->current_transition != transition ||
        atomic_load_explicit(&state->mode, memory_order_acquire) != expected_mode) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("native transition changed while leaving native code");
    }
    state->current_transition = transition->previous;
    state->managed_stack_boundary =
        (const char *)(uintptr_t)transition->managed_stack_high;
    state->managed_depth++;
    state->managed_segment = SCOOP_MANAGED_SEGMENT_ACTIVE;
    atomic_store_explicit(
        &state->observed_gc_epoch,
        atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire),
        memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    transition->previous = NULL;
    transition->caller_roots = NULL;
    transition->managed_return_pc = 0;
    transition->managed_stack_pointer = 0;
    transition->managed_frame_pointer = 0;
    transition->managed_stack_low = 0;
    transition->managed_stack_high = 0;
    transition->previous_mode = 0;
    transition->native_mode = 0;
    scoop_thread_world_broadcast();
    scoop_thread_registry_unlock();
}

void scoop_rt_enter_native_safe_impl(ScoopThreadTransition *transition,
                                     uintptr_t managed_stack_low, uintptr_t return_pc,
                                     uintptr_t stack_pointer, uintptr_t frame_pointer) {
    enter_native(transition, managed_stack_low, return_pc, stack_pointer, frame_pointer,
                 SCOOP_THREAD_NATIVE_SAFE);
}

void scoop_rt_leave_native_safe(ScoopThreadTransition *transition) {
    leave_native(transition, SCOOP_THREAD_NATIVE_SAFE);
}

void scoop_rt_enter_native_borrowed_impl(ScoopThreadTransition *transition,
                                         uintptr_t managed_stack_low,
                                         uintptr_t return_pc, uintptr_t stack_pointer,
                                         uintptr_t frame_pointer) {
    enter_native(transition, managed_stack_low, return_pc, stack_pointer, frame_pointer,
                 SCOOP_THREAD_NATIVE_BORROWED);
}

void scoop_rt_leave_native_borrowed(ScoopThreadTransition *transition) {
    leave_native(transition, SCOOP_THREAD_NATIVE_BORROWED);
}
