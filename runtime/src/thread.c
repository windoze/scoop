#include "thread/internal.h"

pthread_mutex_t scoop_thread_world_lock = PTHREAD_MUTEX_INITIALIZER;
pthread_cond_t scoop_thread_world_changed = PTHREAD_COND_INITIALIZER;
pthread_mutex_t scoop_thread_collector_lock = PTHREAD_MUTEX_INITIALIZER;
ScoopThreadState *scoop_thread_registry;
uint64_t scoop_thread_registry_count;
ScoopRuntimeLifecycle scoop_thread_runtime_lifecycle = SCOOP_RUNTIME_UNINITIALIZED;
_Atomic(ScoopWorldPhase) scoop_thread_world_phase = SCOOP_WORLD_RUNNING;
_Atomic(uint64_t) scoop_thread_gc_epoch;
_Atomic(uint64_t) scoop_thread_last_gc_parked_count;
_Atomic(uint64_t) scoop_thread_last_gc_native_safe_count;
_Thread_local ScoopThreadState *scoop_thread_tls;
_Thread_local ScoopAllocationContext *scoop_rt_allocation_context;

_Noreturn void scoop_thread_fatal(const char *message) {
    fprintf(stderr, "scoop runtime: %s\n", message);
    abort();
}
void scoop_thread_registry_lock(void) {
    if (pthread_mutex_lock(&scoop_thread_world_lock) != 0) {
        scoop_thread_fatal("failed to lock the world");
    }
}
void scoop_thread_registry_unlock(void) {
    if (pthread_mutex_unlock(&scoop_thread_world_lock) != 0) {
        scoop_thread_fatal("failed to unlock the world");
    }
}

void scoop_thread_world_wait(void) {
    if (pthread_cond_wait(&scoop_thread_world_changed, &scoop_thread_world_lock) != 0) {
        scoop_thread_fatal("failed to wait for a world-state change");
    }
}

void scoop_thread_world_broadcast(void) {
    if (pthread_cond_broadcast(&scoop_thread_world_changed) != 0) {
        scoop_thread_fatal("failed to broadcast a world-state change");
    }
}

void scoop_thread_wait_for_running_world(void) {
    while (atomic_load_explicit(&scoop_thread_world_phase, memory_order_acquire) !=
           SCOOP_WORLD_RUNNING) {
        scoop_thread_world_wait();
    }
}

static ScoopThreadState *new_thread_state(ScoopThreadAttachmentKind kind,
                                          ScoopThreadMode mode,
                                          uint64_t managed_depth) {
    ScoopThreadState *state = calloc(1, sizeof *state);
    if (state == NULL) {
        scoop_thread_fatal("out of memory attaching a thread");
    }
    state->os_thread = pthread_self();
    ScoopPlatformStackBounds bounds = scoop_platform_stack_bounds();
    state->stack_low = bounds.low;
    state->stack_high = bounds.high;
    atomic_init(&state->mode, mode);
    atomic_init(&state->observed_gc_epoch,
                atomic_load_explicit(&scoop_thread_gc_epoch, memory_order_acquire));
    state->parked_from = mode;
    state->managed_depth = managed_depth;
    state->attachment_kind = kind;
    return state;
}

static void registry_insert(ScoopThreadState *state) {
    state->registry_prev = NULL;
    state->registry_next = scoop_thread_registry;
    if (scoop_thread_registry != NULL) {
        scoop_thread_registry->registry_prev = state;
    }
    scoop_thread_registry = state;
    scoop_thread_registry_count++;
}

static void registry_remove(ScoopThreadState *state) {
    if (state->registry_prev != NULL) {
        state->registry_prev->registry_next = state->registry_next;
    } else if (scoop_thread_registry == state) {
        scoop_thread_registry = state->registry_next;
    } else {
        scoop_thread_fatal("thread registry is corrupt during detach");
    }
    if (state->registry_next != NULL) {
        state->registry_next->registry_prev = state->registry_prev;
    }
    if (scoop_thread_registry_count == 0) {
        scoop_thread_fatal("thread registry count underflow");
    }
    scoop_thread_registry_count--;
}

static void require_detachable(const ScoopThreadState *state,
                               ScoopThreadAttachmentKind expected_kind) {
    if (state->attachment_kind != expected_kind) {
        scoop_thread_fatal(
            expected_kind == SCOOP_THREAD_MAIN
                ? "attempted to detach a foreign thread as the main thread"
                : "attempted to detach the main thread as a foreign thread");
    }
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
        SCOOP_THREAD_NATIVE_SAFE) {
        scoop_thread_fatal("thread is not in a detachable execution mode");
    }
    if (state->managed_depth != 0 || state->managed_stack_boundary != NULL ||
        state->managed_segment != SCOOP_MANAGED_SEGMENT_NONE ||
        state->callback_depth != 0 || state->native_roots != NULL ||
        state->native_region_roots != NULL || state->caller_roots != NULL ||
        state->compiler_roots != NULL || state->initialization_stack_len != 0 ||
        state->initialization_wait != NULL || state->current_transition != NULL ||
        state->caught_exception_top != NULL || state->managed_anchor != NULL ||
        state->allocation.cursor != NULL || state->allocation.limit != NULL) {
        scoop_thread_fatal("thread detach with active managed frames, callbacks, "
                           "roots, exceptions, or transitions");
    }
}

static void detach_current(ScoopThreadAttachmentKind expected_kind) {
    ScoopThreadState *state = scoop_thread_tls;
    if (state == NULL) {
        scoop_thread_fatal("attempted to detach an unattached thread");
    }

    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    require_detachable(state, expected_kind);
    state->current_task_context = NULL;
    atomic_store_explicit(&state->mode, SCOOP_THREAD_DETACHING, memory_order_release);
    registry_remove(state);
    scoop_thread_tls = NULL;
    scoop_rt_allocation_context = NULL;
    scoop_thread_registry_unlock();
    free(state->initialization_stack);
    free(state->initialization_cycle_path);
    free(state);
}

void scoop_thread_runtime_init(void) {
    scoop_thread_registry_lock();
    if (scoop_thread_runtime_lifecycle != SCOOP_RUNTIME_UNINITIALIZED ||
        scoop_thread_registry_count != 0 || scoop_thread_registry != NULL) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("runtime thread registry initialized more than once");
    }
    atomic_store_explicit(&scoop_thread_world_phase, SCOOP_WORLD_RUNNING,
                          memory_order_release);
    atomic_store_explicit(&scoop_thread_gc_epoch, 0, memory_order_release);
    atomic_store_explicit(&scoop_thread_last_gc_parked_count, 0, memory_order_release);
    atomic_store_explicit(&scoop_thread_last_gc_native_safe_count, 0,
                          memory_order_release);
    scoop_thread_runtime_lifecycle = SCOOP_RUNTIME_RUNNING;
    scoop_thread_registry_unlock();
}

void scoop_thread_attach_main(void) {
    if (scoop_thread_tls != NULL) {
        scoop_thread_fatal("main thread is already attached");
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_MAIN, SCOOP_THREAD_NATIVE_SAFE, 0);

    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    if (scoop_thread_runtime_lifecycle != SCOOP_RUNTIME_RUNNING ||
        scoop_thread_registry_count != 0) {
        scoop_thread_registry_unlock();
        free(state);
        scoop_thread_fatal("main thread must be the first runtime attachment");
    }
    registry_insert(state);
    scoop_thread_tls = state;
    scoop_rt_allocation_context = &state->allocation;
    scoop_thread_registry_unlock();
}

bool scoop_rt_attach_foreign_thread(void) {
    if (scoop_thread_tls != NULL) {
        return false;
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_FOREIGN, SCOOP_THREAD_NATIVE_SAFE, 0);

    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    if (scoop_thread_runtime_lifecycle != SCOOP_RUNTIME_RUNNING) {
        scoop_thread_registry_unlock();
        free(state);
        scoop_thread_fatal("foreign thread attach after runtime shutdown began");
    }
    registry_insert(state);
    scoop_thread_tls = state;
    scoop_rt_allocation_context = &state->allocation;
    scoop_thread_registry_unlock();
    return true;
}

void scoop_rt_detach_foreign_thread(void) { detach_current(SCOOP_THREAD_FOREIGN); }

void scoop_thread_prepare_shutdown(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (state->attachment_kind != SCOOP_THREAD_MAIN) {
        scoop_thread_fatal("runtime shutdown requested from a foreign thread");
    }

    scoop_thread_registry_lock();
    scoop_thread_wait_for_running_world();
    if (scoop_thread_runtime_lifecycle != SCOOP_RUNTIME_RUNNING) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("runtime shutdown entered from an invalid lifecycle state");
    }
    scoop_thread_runtime_lifecycle = SCOOP_RUNTIME_SHUTTING_DOWN;
    if (scoop_thread_registry_count != 1 || scoop_thread_registry != state) {
        uint64_t attached = scoop_thread_registry_count;
        scoop_thread_registry_unlock();
        fprintf(stderr, "scoop runtime: shutdown with %" PRIu64 " attached thread(s)\n",
                attached);
        abort();
    }
    /* Managed exit already retired the TLAB. Shutdown itself stays native-safe. */
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    scoop_thread_registry_unlock();
}

void scoop_thread_detach_main(void) { detach_current(SCOOP_THREAD_MAIN); }

void scoop_thread_runtime_finish_shutdown(void) {
    scoop_thread_registry_lock();
    if (scoop_thread_runtime_lifecycle != SCOOP_RUNTIME_SHUTTING_DOWN ||
        scoop_thread_registry_count != 0 || scoop_thread_registry != NULL) {
        scoop_thread_registry_unlock();
        scoop_thread_fatal("runtime thread registry did not drain during shutdown");
    }
    scoop_thread_runtime_lifecycle = SCOOP_RUNTIME_STOPPED;
    scoop_thread_registry_unlock();
}

ScoopThreadState *scoop_thread_current(void) { return scoop_thread_tls; }

ScoopThreadState *scoop_thread_current_required(void) {
    ScoopThreadState *state = scoop_thread_tls;
    if (state == NULL) {
        scoop_thread_fatal("unattached thread entered the Scoop runtime");
    }
    return state;
}

void scoop_thread_require_managed(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_MANAGED ||
        state->managed_depth == 0 ||
        state->managed_segment != SCOOP_MANAGED_SEGMENT_ACTIVE) {
        scoop_thread_fatal("thread entered managed code from a non-managed state");
    }
}
