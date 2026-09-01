#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <inttypes.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>

#include "scoop_rt.h"
#include "thread.h"

typedef enum ScoopRuntimeLifecycle {
    SCOOP_RUNTIME_UNINITIALIZED,
    SCOOP_RUNTIME_RUNNING,
    SCOOP_RUNTIME_SHUTTING_DOWN,
    SCOOP_RUNTIME_STOPPED,
} ScoopRuntimeLifecycle;

typedef enum ScoopWorldPhase {
    SCOOP_WORLD_RUNNING,
    SCOOP_WORLD_STOPPING,
    SCOOP_WORLD_COLLECTING,
} ScoopWorldPhase;

static pthread_mutex_t world_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t world_changed = PTHREAD_COND_INITIALIZER;
static pthread_mutex_t collector_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopThreadState *thread_registry_head;
static uint64_t thread_registry_count;
static ScoopRuntimeLifecycle runtime_lifecycle = SCOOP_RUNTIME_UNINITIALIZED;
static _Atomic(ScoopWorldPhase) world_phase = SCOOP_WORLD_RUNNING;
static _Atomic(uint64_t) gc_epoch;
static _Atomic(uint64_t) last_gc_parked_count;
static _Atomic(uint64_t) last_gc_native_safe_count;
static _Thread_local ScoopThreadState *current_thread;
_Thread_local ScoopAllocationContext *scoop_rt_allocation_context;

static _Noreturn void thread_fatal(const char *message) {
    fprintf(stderr, "scoop runtime: %s\n", message);
    abort();
}

static void registry_lock(void) {
    if (pthread_mutex_lock(&world_lock) != 0) {
        thread_fatal("failed to lock the world");
    }
}

static void registry_unlock(void) {
    if (pthread_mutex_unlock(&world_lock) != 0) {
        thread_fatal("failed to unlock the world");
    }
}

static void world_wait(void) {
    if (pthread_cond_wait(&world_changed, &world_lock) != 0) {
        thread_fatal("failed to wait for a world-state change");
    }
}

static void world_broadcast(void) {
    if (pthread_cond_broadcast(&world_changed) != 0) {
        thread_fatal("failed to broadcast a world-state change");
    }
}

static void wait_for_running_world(void) {
    while (atomic_load_explicit(&world_phase, memory_order_acquire) !=
           SCOOP_WORLD_RUNNING) {
        world_wait();
    }
}

static void read_stack_bounds(const char **low, const char **high) {
#if defined(__APPLE__)
    pthread_t self = pthread_self();
    void *stack_high = pthread_get_stackaddr_np(self);
    size_t stack_size = pthread_get_stacksize_np(self);
    if (stack_high == NULL || stack_size == 0) {
        thread_fatal("failed to read pthread stack bounds");
    }
    *high = stack_high;
    *low = (const char *)stack_high - stack_size;
#elif defined(__linux__)
    pthread_attr_t attributes;
    void *stack_low = NULL;
    size_t stack_size = 0;
    if (pthread_getattr_np(pthread_self(), &attributes) != 0) {
        thread_fatal("failed to read pthread attributes");
    }
    int stack_result = pthread_attr_getstack(&attributes, &stack_low, &stack_size);
    int destroy_result = pthread_attr_destroy(&attributes);
    if (stack_result != 0 || destroy_result != 0 || stack_low == NULL || stack_size == 0) {
        thread_fatal("failed to read pthread stack bounds");
    }
    *low = stack_low;
    *high = (const char *)stack_low + stack_size;
#else
#error "M13 thread registration currently requires a POSIX pthread host"
#endif
}

static ScoopThreadState *new_thread_state(ScoopThreadAttachmentKind kind,
                                          ScoopThreadMode mode,
                                          uint64_t managed_depth) {
    ScoopThreadState *state = calloc(1, sizeof *state);
    if (state == NULL) {
        thread_fatal("out of memory attaching a thread");
    }
    state->os_thread = pthread_self();
    read_stack_bounds(&state->stack_low, &state->stack_high);
    atomic_init(&state->mode, mode);
    atomic_init(&state->observed_gc_epoch,
                atomic_load_explicit(&gc_epoch, memory_order_acquire));
    state->parked_from = mode;
    state->managed_depth = managed_depth;
    state->attachment_kind = kind;
    return state;
}

static void registry_insert(ScoopThreadState *state) {
    state->registry_prev = NULL;
    state->registry_next = thread_registry_head;
    if (thread_registry_head != NULL) {
        thread_registry_head->registry_prev = state;
    }
    thread_registry_head = state;
    thread_registry_count++;
}

static void registry_remove(ScoopThreadState *state) {
    if (state->registry_prev != NULL) {
        state->registry_prev->registry_next = state->registry_next;
    } else if (thread_registry_head == state) {
        thread_registry_head = state->registry_next;
    } else {
        thread_fatal("thread registry is corrupt during detach");
    }
    if (state->registry_next != NULL) {
        state->registry_next->registry_prev = state->registry_prev;
    }
    if (thread_registry_count == 0) {
        thread_fatal("thread registry count underflow");
    }
    thread_registry_count--;
}

static void require_detachable(const ScoopThreadState *state,
                               ScoopThreadAttachmentKind expected_kind) {
    if (state->attachment_kind != expected_kind) {
        thread_fatal(expected_kind == SCOOP_THREAD_MAIN
                         ? "attempted to detach a foreign thread as the main thread"
                         : "attempted to detach the main thread as a foreign thread");
    }
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
        (expected_kind == SCOOP_THREAD_MAIN ? SCOOP_THREAD_MANAGED
                                            : SCOOP_THREAD_NATIVE_SAFE)) {
        thread_fatal("thread is not in a detachable execution mode");
    }
    if (state->managed_depth != (expected_kind == SCOOP_THREAD_MAIN ? 1 : 0) ||
        state->callback_depth != 0 || state->native_roots != NULL ||
        state->caller_roots != NULL || state->current_transition != NULL ||
        state->allocation.cursor != NULL || state->allocation.limit != NULL) {
        thread_fatal(
            "thread detach with active managed frames, callbacks, roots, or transitions");
    }
}

static void detach_current(ScoopThreadAttachmentKind expected_kind) {
    ScoopThreadState *state = current_thread;
    if (state == NULL) {
        thread_fatal("attempted to detach an unattached thread");
    }

    registry_lock();
    wait_for_running_world();
    require_detachable(state, expected_kind);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_DETACHING, memory_order_release);
    registry_remove(state);
    current_thread = NULL;
    scoop_rt_allocation_context = NULL;
    registry_unlock();
    free(state);
}

void scoop_thread_runtime_init(void) {
    registry_lock();
    if (runtime_lifecycle != SCOOP_RUNTIME_UNINITIALIZED || thread_registry_count != 0 ||
        thread_registry_head != NULL) {
        registry_unlock();
        thread_fatal("runtime thread registry initialized more than once");
    }
    atomic_store_explicit(&world_phase, SCOOP_WORLD_RUNNING, memory_order_release);
    atomic_store_explicit(&gc_epoch, 0, memory_order_release);
    atomic_store_explicit(&last_gc_parked_count, 0, memory_order_release);
    atomic_store_explicit(&last_gc_native_safe_count, 0, memory_order_release);
    runtime_lifecycle = SCOOP_RUNTIME_RUNNING;
    registry_unlock();
}

void scoop_thread_attach_main(const void *managed_stack_boundary) {
    if (current_thread != NULL) {
        thread_fatal("main thread is already attached");
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_MAIN, SCOOP_THREAD_MANAGED, 1);
    uintptr_t boundary = (uintptr_t)managed_stack_boundary;
    if (boundary < (uintptr_t)state->stack_low || boundary > (uintptr_t)state->stack_high) {
        thread_fatal("main thread published an invalid managed stack boundary");
    }
    state->managed_stack_boundary = managed_stack_boundary;

    registry_lock();
    wait_for_running_world();
    if (runtime_lifecycle != SCOOP_RUNTIME_RUNNING || thread_registry_count != 0) {
        registry_unlock();
        free(state);
        thread_fatal("main thread must be the first runtime attachment");
    }
    registry_insert(state);
    current_thread = state;
    scoop_rt_allocation_context = &state->allocation;
    registry_unlock();
}

bool scoop_rt_attach_foreign_thread(void) {
    if (current_thread != NULL) {
        return false;
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_FOREIGN, SCOOP_THREAD_NATIVE_SAFE, 0);

    registry_lock();
    wait_for_running_world();
    if (runtime_lifecycle != SCOOP_RUNTIME_RUNNING) {
        registry_unlock();
        free(state);
        thread_fatal("foreign thread attach after runtime shutdown began");
    }
    registry_insert(state);
    current_thread = state;
    scoop_rt_allocation_context = &state->allocation;
    registry_unlock();
    return true;
}

void scoop_rt_detach_foreign_thread(void) {
    detach_current(SCOOP_THREAD_FOREIGN);
}

void scoop_thread_prepare_shutdown(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (state->attachment_kind != SCOOP_THREAD_MAIN) {
        thread_fatal("runtime shutdown requested from a foreign thread");
    }

    registry_lock();
    wait_for_running_world();
    if (runtime_lifecycle != SCOOP_RUNTIME_RUNNING) {
        registry_unlock();
        thread_fatal("runtime shutdown entered from an invalid lifecycle state");
    }
    runtime_lifecycle = SCOOP_RUNTIME_SHUTTING_DOWN;
    if (thread_registry_count != 1 || thread_registry_head != state) {
        uint64_t attached = thread_registry_count;
        registry_unlock();
        fprintf(stderr,
                "scoop runtime: shutdown with %" PRIu64 " attached thread(s)\n",
                attached);
        abort();
    }
    /* Shutdown is the main thread's final managed boundary. Retire its
     * owner-only TLAB before detach; the remaining tail is reclaimed only if
     * a final collection is requested, but must never survive attachment. */
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    registry_unlock();
}

void scoop_thread_detach_main(void) {
    detach_current(SCOOP_THREAD_MAIN);
}

void scoop_thread_runtime_finish_shutdown(void) {
    registry_lock();
    if (runtime_lifecycle != SCOOP_RUNTIME_SHUTTING_DOWN || thread_registry_count != 0 ||
        thread_registry_head != NULL) {
        registry_unlock();
        thread_fatal("runtime thread registry did not drain during shutdown");
    }
    runtime_lifecycle = SCOOP_RUNTIME_STOPPED;
    registry_unlock();
}

ScoopThreadState *scoop_thread_current(void) {
    return current_thread;
}

ScoopThreadState *scoop_thread_current_required(void) {
    ScoopThreadState *state = current_thread;
    if (state == NULL) {
        thread_fatal("unattached thread entered the Scoop runtime");
    }
    return state;
}

void scoop_thread_require_managed(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != SCOOP_THREAD_MANAGED ||
        state->managed_depth == 0) {
        thread_fatal("thread entered managed code from a non-managed state");
    }
}

static void spill_and_park_current_locked(ScoopThreadState *state,
                                          const char *stack_pointer) {
    ScoopThreadMode from = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (from != SCOOP_THREAD_MANAGED && from != SCOOP_THREAD_NATIVE_BORROWED) {
        thread_fatal("only managed or native-borrowed threads may park");
    }

    /* These non-atomic fields are the collector's immutable snapshot for the
     * whole parked interval. A condition-variable wait may wake spuriously
     * while the collector scans outside world_lock, so never rewrite them in
     * the acknowledgement loop below. */
    state->parked_from = from;
    state->parked_sp = stack_pointer;
    while (atomic_load_explicit(&world_phase, memory_order_acquire) !=
           SCOOP_WORLD_RUNNING) {
        uint64_t epoch = atomic_load_explicit(&gc_epoch, memory_order_acquire);
        atomic_store_explicit(&state->observed_gc_epoch, epoch, memory_order_release);
        atomic_store_explicit(&state->mode, SCOOP_THREAD_PARKED, memory_order_release);
        world_broadcast();
        /* Wait once rather than hiding phase changes in wait_for_running_world:
         * a new collector may win the world lock before an old-epoch parker
         * wakes. The outer loop must then acknowledge the new epoch while the
         * same spill/SP are still valid. */
        world_wait();
    }

    state->parked_sp = NULL;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, from, memory_order_release);
}

void scoop_thread_poll(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    uint64_t epoch = atomic_load_explicit(&gc_epoch, memory_order_acquire);
    if (atomic_load_explicit(&world_phase, memory_order_acquire) ==
            SCOOP_WORLD_RUNNING &&
        atomic_load_explicit(&state->observed_gc_epoch, memory_order_acquire) == epoch) {
        return;
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    } else {
        atomic_store_explicit(&state->observed_gc_epoch,
                              atomic_load_explicit(&gc_epoch, memory_order_acquire),
                              memory_order_release);
    }
    registry_unlock();
}

void scoop_thread_runtime_entry(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_acquire);
    if (mode == SCOOP_THREAD_MANAGED) {
        scoop_thread_poll();
        return;
    }
    if (mode != SCOOP_THREAD_NATIVE_BORROWED) {
        thread_fatal("managed runtime entry from an invalid native mode");
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    registry_unlock();
}

static bool all_collection_targets_quiescent(ScoopThreadState *collector,
                                             uint64_t epoch,
                                             uint64_t *parked_count,
                                             uint64_t *native_safe_count) {
    uint64_t parked = 0;
    uint64_t native_safe = 0;
    for (ScoopThreadState *state = thread_registry_head; state != NULL;
         state = state->registry_next) {
        if (state == collector) {
            continue;
        }
        ScoopThreadMode mode = atomic_load_explicit(&state->mode, memory_order_acquire);
        if (mode == SCOOP_THREAD_NATIVE_SAFE) {
            native_safe++;
            continue;
        }
        if (mode == SCOOP_THREAD_PARKED &&
            atomic_load_explicit(&state->observed_gc_epoch, memory_order_acquire) ==
                epoch) {
            parked++;
            continue;
        }
        if (mode == SCOOP_THREAD_MANAGED || mode == SCOOP_THREAD_NATIVE_BORROWED ||
            mode == SCOOP_THREAD_PARKED) {
            return false;
        }
        thread_fatal("invalid thread mode while stopping the world");
    }
    *parked_count = parked;
    *native_safe_count = native_safe;
    return true;
}

bool scoop_thread_begin_collection(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode requester_mode =
        atomic_load_explicit(&state->mode, memory_order_acquire);
    if ((requester_mode != SCOOP_THREAD_MANAGED &&
         requester_mode != SCOOP_THREAD_NATIVE_BORROWED) ||
        (requester_mode == SCOOP_THREAD_MANAGED && state->managed_depth == 0)) {
        thread_fatal("collection requested from an invalid thread mode");
    }
    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);

    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
        registry_unlock();
        return false;
    }
    if (pthread_mutex_lock(&collector_lock) != 0) {
        registry_unlock();
        thread_fatal("failed to lock the collector");
    }

    uint64_t epoch =
        atomic_fetch_add_explicit(&gc_epoch, 1, memory_order_acq_rel) + 1;
    atomic_store_explicit(&world_phase, SCOOP_WORLD_STOPPING, memory_order_release);
    state->parked_from = requester_mode;
    state->parked_sp = (const char *)&stack_marker;
    atomic_store_explicit(&state->observed_gc_epoch, epoch, memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_COLLECTOR, memory_order_release);
    world_broadcast();

    uint64_t parked_count = 0;
    uint64_t native_safe_count = 0;
    while (!all_collection_targets_quiescent(state, epoch, &parked_count,
                                              &native_safe_count)) {
        world_wait();
    }
    atomic_store_explicit(&last_gc_parked_count, parked_count, memory_order_release);
    atomic_store_explicit(&last_gc_native_safe_count, native_safe_count,
                          memory_order_release);
    atomic_store_explicit(&world_phase, SCOOP_WORLD_COLLECTING, memory_order_release);
    registry_unlock();
    return true;
}

void scoop_thread_end_collection(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
        SCOOP_THREAD_COLLECTOR) {
        thread_fatal("collection ended by a non-collector thread");
    }

    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_COLLECTING) {
        registry_unlock();
        thread_fatal("collection ended outside the collecting phase");
    }
    ScoopThreadMode requester_mode = state->parked_from;
    state->parked_sp = NULL;
    atomic_store_explicit(&state->mode, requester_mode, memory_order_release);
    if (pthread_mutex_unlock(&collector_lock) != 0) {
        registry_unlock();
        thread_fatal("failed to unlock the collector");
    }
    atomic_store_explicit(&world_phase, SCOOP_WORLD_RUNNING, memory_order_release);
    world_broadcast();
    registry_unlock();
}

ScoopThreadState *scoop_thread_collection_registry_head(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_COLLECTOR ||
        atomic_load_explicit(&world_phase, memory_order_acquire) !=
            SCOOP_WORLD_COLLECTING) {
        thread_fatal("thread registry enumerated outside STW collection");
    }
    return thread_registry_head;
}

void scoop_thread_enter_managed(const void *managed_stack_boundary) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_NATIVE_SAFE ||
        state->managed_depth != 0) {
        thread_fatal("invalid managed entry transition");
    }
    uintptr_t boundary = (uintptr_t)managed_stack_boundary;
    if (boundary < (uintptr_t)state->stack_low || boundary > (uintptr_t)state->stack_high) {
        thread_fatal("managed entry published an invalid stack boundary");
    }
    registry_lock();
    wait_for_running_world();
    state->managed_depth = 1;
    state->managed_stack_boundary = managed_stack_boundary;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    registry_unlock();
}

void scoop_thread_leave_managed(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (state->managed_depth != 1) {
        thread_fatal("invalid managed exit transition");
    }
    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    state->managed_depth = 0;
    state->managed_stack_boundary = NULL;
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    atomic_store_explicit(&state->mode, SCOOP_THREAD_NATIVE_SAFE, memory_order_release);
    registry_unlock();
}

void scoop_thread_enter_callback(ScoopCallbackThreadEntry *entry,
                                 const void *managed_stack_boundary) {
    ScoopThreadState *state = scoop_thread_current_required();
    ScoopThreadMode previous =
        atomic_load_explicit(&state->mode, memory_order_acquire);
    if (entry == NULL || entry->active ||
        (previous != SCOOP_THREAD_NATIVE_SAFE &&
         previous != SCOOP_THREAD_NATIVE_BORROWED)) {
        thread_fatal("invalid managed callback entry transition");
    }
    uintptr_t boundary = (uintptr_t)managed_stack_boundary;
    if (boundary < (uintptr_t)state->stack_low ||
        boundary > (uintptr_t)state->stack_high) {
        thread_fatal("managed callback published an invalid stack boundary");
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (previous == SCOOP_THREAD_NATIVE_SAFE) {
        wait_for_running_world();
    } else if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
               SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    if (atomic_load_explicit(&state->mode, memory_order_acquire) != previous) {
        registry_unlock();
        thread_fatal("native mode changed during managed callback entry");
    }
    entry->previous_mode = previous;
    entry->previous_managed_stack_boundary = state->managed_stack_boundary;
    entry->previous_managed_depth = state->managed_depth;
    entry->active = true;
    state->callback_depth++;
    state->managed_depth++;
    state->managed_stack_boundary = managed_stack_boundary;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED,
                          memory_order_release);
    registry_unlock();
}

void scoop_thread_leave_callback(ScoopCallbackThreadEntry *entry) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (entry == NULL || !entry->active || state->callback_depth == 0 ||
        state->managed_depth != entry->previous_managed_depth + 1) {
        thread_fatal("invalid managed callback exit transition");
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    state->allocation.cursor = NULL;
    state->allocation.limit = NULL;
    state->callback_depth--;
    state->managed_depth = entry->previous_managed_depth;
    state->managed_stack_boundary = entry->previous_managed_stack_boundary;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, entry->previous_mode,
                          memory_order_release);
    entry->active = false;
    registry_unlock();
}

void scoop_rt_push_caller_roots(ScoopCallerRootFrame *frame,
                                ScoopCallerRootEntry *entries,
                                uint64_t count) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL) {
        thread_fatal("invalid caller root frame");
    }
    if (count != 0 && entries == NULL) {
        thread_fatal("invalid caller root frame");
    }
    for (uint64_t i = 0; i < count; i++) {
        if (entries[i].base == NULL || entries[i].scan == NULL) {
            thread_fatal("invalid caller root frame");
        }
    }
    for (ScoopCallerRootFrame *active = state->caller_roots; active != NULL;
         active = active->previous) {
        if (active == frame) {
            thread_fatal("caller root frame is already active");
        }
    }
    frame->previous = state->caller_roots;
    frame->entries = entries;
    frame->count = count;
    state->caller_roots = frame;
}

void scoop_rt_pop_caller_roots(ScoopCallerRootFrame *frame) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    if (frame == NULL || state->caller_roots != frame) {
        thread_fatal("caller root frames must be popped in LIFO order");
    }
    ScoopCallerRootFrame *previous = frame->previous;
    state->caller_roots = previous;
    frame->previous = NULL;
    frame->entries = NULL;
    frame->count = 0;
}

static void enter_native(ScoopThreadTransition *transition,
                         uintptr_t managed_stack_pointer,
                         ScoopThreadMode native_mode) {
    ScoopThreadState *state = scoop_thread_current_required();
    scoop_thread_require_managed();
    uintptr_t managed_boundary = (uintptr_t)state->managed_stack_boundary;
    if (transition == NULL || managed_stack_pointer < (uintptr_t)state->stack_low ||
        managed_stack_pointer >= managed_boundary ||
        managed_boundary > (uintptr_t)state->stack_high) {
        thread_fatal("native transition published an invalid managed stack segment");
    }
    for (ScoopThreadTransition *active = state->current_transition; active != NULL;
         active = active->previous) {
        if (active == transition) {
            thread_fatal("native transition record is already active");
        }
    }
    if (state->caller_roots == NULL) {
        thread_fatal("native transition requires a caller root frame");
    }
    if (state->current_transition != NULL &&
        state->current_transition->caller_roots == state->caller_roots) {
        thread_fatal("native transition requires a fresh caller root frame");
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
        SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    if (atomic_load_explicit(&state->mode, memory_order_acquire) !=
            SCOOP_THREAD_MANAGED ||
        state->managed_depth == 0) {
        registry_unlock();
        thread_fatal("native transition lost its managed source state");
    }
    transition->previous = state->current_transition;
    transition->caller_roots = state->caller_roots;
    transition->managed_stack_low = managed_stack_pointer;
    transition->managed_stack_high = managed_boundary;
    transition->previous_mode = SCOOP_THREAD_MANAGED;
    transition->native_mode = native_mode;
    state->current_transition = transition;
    state->managed_stack_boundary = NULL;
    state->managed_depth--;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, native_mode, memory_order_release);
    registry_unlock();
}

static void leave_native(ScoopThreadTransition *transition,
                         ScoopThreadMode expected_mode) {
    ScoopThreadState *state = scoop_thread_current_required();
    if (transition == NULL || state->current_transition != transition ||
        atomic_load_explicit(&state->mode, memory_order_acquire) != expected_mode ||
        transition->native_mode != (uint32_t)expected_mode ||
        transition->previous_mode != (uint32_t)SCOOP_THREAD_MANAGED) {
        thread_fatal("native transitions must be left in LIFO order");
    }

    volatile char stack_marker = 0;
    (void)setjmp(state->register_spill);
    registry_lock();
    if (expected_mode == SCOOP_THREAD_NATIVE_SAFE) {
        wait_for_running_world();
    } else if (atomic_load_explicit(&world_phase, memory_order_acquire) !=
               SCOOP_WORLD_RUNNING) {
        spill_and_park_current_locked(state, (const char *)&stack_marker);
    }
    if (state->current_transition != transition ||
        atomic_load_explicit(&state->mode, memory_order_acquire) != expected_mode) {
        registry_unlock();
        thread_fatal("native transition changed while leaving native code");
    }
    state->current_transition = transition->previous;
    state->managed_stack_boundary =
        (const char *)(uintptr_t)transition->managed_stack_high;
    state->managed_depth++;
    atomic_store_explicit(&state->observed_gc_epoch,
                          atomic_load_explicit(&gc_epoch, memory_order_acquire),
                          memory_order_release);
    atomic_store_explicit(&state->mode, SCOOP_THREAD_MANAGED, memory_order_release);
    transition->previous = NULL;
    transition->caller_roots = NULL;
    transition->managed_stack_low = 0;
    transition->managed_stack_high = 0;
    transition->previous_mode = 0;
    transition->native_mode = 0;
    registry_unlock();
}

void scoop_rt_enter_native_safe(ScoopThreadTransition *transition,
                                uintptr_t managed_stack_pointer) {
    enter_native(transition, managed_stack_pointer, SCOOP_THREAD_NATIVE_SAFE);
}

void scoop_rt_leave_native_safe(ScoopThreadTransition *transition) {
    leave_native(transition, SCOOP_THREAD_NATIVE_SAFE);
}

void scoop_rt_enter_native_borrowed(ScoopThreadTransition *transition,
                                    uintptr_t managed_stack_pointer) {
    enter_native(transition, managed_stack_pointer, SCOOP_THREAD_NATIVE_BORROWED);
}

void scoop_rt_leave_native_borrowed(ScoopThreadTransition *transition) {
    leave_native(transition, SCOOP_THREAD_NATIVE_BORROWED);
}

bool scoop_rt_thread_debug_is_attached(void) {
    return current_thread != NULL;
}

uint32_t scoop_rt_thread_debug_mode(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    return (uint32_t)atomic_load_explicit(&state->mode, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_count(void) {
    registry_lock();
    uint64_t count = thread_registry_count;
    registry_unlock();
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

void scoop_rt_thread_debug_leave_managed(void) {
    scoop_thread_leave_managed();
}

uint64_t scoop_rt_thread_debug_gc_epoch(void) {
    return atomic_load_explicit(&gc_epoch, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_last_gc_parked_count(void) {
    return atomic_load_explicit(&last_gc_parked_count, memory_order_acquire);
}

uint64_t scoop_rt_thread_debug_last_gc_native_safe_count(void) {
    return atomic_load_explicit(&last_gc_native_safe_count, memory_order_acquire);
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

uint64_t scoop_rt_thread_debug_transition_depth(void) {
    ScoopThreadState *state = scoop_thread_current_required();
    uint64_t depth = 0;
    for (ScoopThreadTransition *transition = state->current_transition;
         transition != NULL; transition = transition->previous) {
        depth++;
    }
    return depth;
}
