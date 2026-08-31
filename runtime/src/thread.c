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

static pthread_mutex_t thread_registry_lock = PTHREAD_MUTEX_INITIALIZER;
static ScoopThreadState *thread_registry_head;
static uint64_t thread_registry_count;
static ScoopRuntimeLifecycle runtime_lifecycle = SCOOP_RUNTIME_UNINITIALIZED;
static _Thread_local ScoopThreadState *current_thread;

static _Noreturn void thread_fatal(const char *message) {
    fprintf(stderr, "scoop runtime: %s\n", message);
    abort();
}

static void registry_lock(void) {
    if (pthread_mutex_lock(&thread_registry_lock) != 0) {
        thread_fatal("failed to lock the thread registry");
    }
}

static void registry_unlock(void) {
    if (pthread_mutex_unlock(&thread_registry_lock) != 0) {
        thread_fatal("failed to unlock the thread registry");
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
        state->callback_depth != 0 || state->native_roots != NULL) {
        thread_fatal("thread detach with active managed frames, callbacks, or native roots");
    }
}

static void detach_current(ScoopThreadAttachmentKind expected_kind) {
    ScoopThreadState *state = current_thread;
    if (state == NULL) {
        thread_fatal("attempted to detach an unattached thread");
    }
    require_detachable(state, expected_kind);

    registry_lock();
    atomic_store_explicit(&state->mode, SCOOP_THREAD_DETACHING, memory_order_release);
    registry_remove(state);
    current_thread = NULL;
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
    runtime_lifecycle = SCOOP_RUNTIME_RUNNING;
    registry_unlock();
}

void scoop_thread_attach_main(void) {
    if (current_thread != NULL) {
        thread_fatal("main thread is already attached");
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_MAIN, SCOOP_THREAD_MANAGED, 1);

    registry_lock();
    if (runtime_lifecycle != SCOOP_RUNTIME_RUNNING || thread_registry_count != 0) {
        registry_unlock();
        free(state);
        thread_fatal("main thread must be the first runtime attachment");
    }
    registry_insert(state);
    current_thread = state;
    registry_unlock();
}

bool scoop_rt_attach_foreign_thread(void) {
    if (current_thread != NULL) {
        return false;
    }
    ScoopThreadState *state =
        new_thread_state(SCOOP_THREAD_FOREIGN, SCOOP_THREAD_NATIVE_SAFE, 0);

    registry_lock();
    if (runtime_lifecycle != SCOOP_RUNTIME_RUNNING) {
        registry_unlock();
        free(state);
        thread_fatal("foreign thread attach after runtime shutdown began");
    }
    registry_insert(state);
    current_thread = state;
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

void scoop_thread_require_single_managed_mutator(void) {
    scoop_thread_require_managed();
    registry_lock();
    bool single = thread_registry_count == 1;
    registry_unlock();
    if (!single) {
        thread_fatal("GC entered before multi-mutator coordination is enabled");
    }
}

bool scoop_rt_thread_debug_is_attached(void) {
    return current_thread != NULL;
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
