#include "internal.h"

bool scoop_rt_thread_debug_is_attached(void) { return scoop_thread_tls != NULL; }
uint32_t scoop_rt_thread_debug_mode(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_acquire);
    switch (mode) {
    case SCOOP_THREAD_MANAGED_PENDING:
        return SCOOP_THREAD_DEBUG_MANAGED;
    case SCOOP_THREAD_NATIVE_SAFE_RETURNING:
        return SCOOP_THREAD_DEBUG_NATIVE_SAFE;
    default:
        return (uint32_t)mode;
    }
}

uint64_t scoop_rt_thread_debug_count(void) {
    scoop_thread_registry_lock();
    uint64_t count = scoop_thread_registry_count;
    scoop_thread_registry_unlock();
    return count;
}

uintptr_t scoop_rt_thread_debug_stack_low(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    return (uintptr_t)state->stack_low;
}

uintptr_t scoop_rt_thread_debug_stack_high(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    return (uintptr_t)state->stack_high;
}

void scoop_rt_thread_debug_enter_managed(uintptr_t managed_stack_boundary) {
    scoop_thread_enter_managed((const void *)managed_stack_boundary);
}

void scoop_rt_thread_debug_leave_managed(void) { scoop_thread_leave_managed(); }

uint64_t scoop_rt_thread_debug_gc_epoch(void) {
    return atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_last_gc_parked_count(void) {
    return atomic_load_explicit(&scoop_thread_last_gc_parked_count, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_last_gc_native_safe_count(void) {
    return atomic_load_explicit(&scoop_thread_last_gc_native_safe_count, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_caller_root_count(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    uint64_t count = 0;
    for (ScoopCallerRootFrame *frame = state->caller_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
}

uint64_t scoop_rt_thread_debug_compiler_root_count(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    uint64_t count = 0;
    for (ScoopCompilerRootFrame *frame = state->compiler_roots; frame != NULL;
         frame = frame->previous) {
        count += frame->count;
    }
    return count;
}

uint64_t scoop_rt_thread_debug_transition_depth(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    uint64_t depth = 0;
    for (ScoopThreadTransition *transition = state->current_transition; transition != NULL;
         transition = transition->previous) {
        depth++;
    }
    return depth;
}
