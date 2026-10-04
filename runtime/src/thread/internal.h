#ifndef SCOOP_RT_THREAD_INTERNAL_H
#define SCOOP_RT_THREAD_INTERNAL_H

#include <inttypes.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>

#include "../thread.h"
#include "scoop_rt.h"

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

extern pthread_mutex_t scoop_thread_world_lock;
extern pthread_cond_t scoop_thread_world_changed;
extern pthread_mutex_t scoop_thread_collector_lock;
extern ScoopThreadState *scoop_thread_registry;
extern uint64_t scoop_thread_registry_count;
extern ScoopRuntimeLifecycle scoop_thread_runtime_lifecycle;
extern _Atomic(ScoopWorldPhase) scoop_thread_world_phase;
extern _Atomic(uint64_t) scoop_thread_gc_epoch;
extern _Atomic(uint64_t) scoop_thread_last_gc_parked_count;
extern _Atomic(uint64_t) scoop_thread_last_gc_native_safe_count;
extern _Thread_local ScoopThreadState *scoop_thread_tls;

_Noreturn void scoop_thread_fatal(const char *message);
void scoop_thread_registry_lock(void);
void scoop_thread_registry_unlock(void);
void scoop_thread_world_wait(void);
void scoop_thread_world_broadcast(void);
void scoop_thread_wait_for_running_world(void);
void scoop_thread_park_current_locked(ScoopThreadState *state);
void scoop_thread_ensure_stack_range(ScoopThreadState *state, uintptr_t low,
                                     uintptr_t high);

#endif /* SCOOP_RT_THREAD_INTERNAL_H */
