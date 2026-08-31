#ifndef SCOOP_RT_THREAD_H
#define SCOOP_RT_THREAD_H

#include <pthread.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"

typedef enum ScoopThreadMode {
    SCOOP_THREAD_NATIVE_SAFE,
    SCOOP_THREAD_MANAGED,
    SCOOP_THREAD_NATIVE_BORROWED,
    SCOOP_THREAD_PARKED,
    SCOOP_THREAD_COLLECTOR,
    SCOOP_THREAD_DETACHING,
} ScoopThreadMode;

typedef enum ScoopThreadAttachmentKind {
    SCOOP_THREAD_MAIN,
    SCOOP_THREAD_FOREIGN,
} ScoopThreadAttachmentKind;

/* Runtime-private per-OS-thread state. The address is stable from registry
 * insertion until detach. M13's later STW/TLAB gates extend this same entity;
 * they must not reintroduce separate main-thread globals. */
typedef struct ScoopThreadState {
    pthread_t os_thread;
    const char *stack_low;
    const char *stack_high;
    _Atomic(ScoopThreadMode) mode;
    uint64_t managed_depth;
    uint64_t callback_depth;
    ScoopThreadAttachmentKind attachment_kind;
    ScoopNativeRootFrame *native_roots;
    struct ScoopThreadState *registry_prev;
    struct ScoopThreadState *registry_next;
} ScoopThreadState;

void scoop_thread_runtime_init(void);
void scoop_thread_attach_main(void);
void scoop_thread_prepare_shutdown(void);
void scoop_thread_detach_main(void);
void scoop_thread_runtime_finish_shutdown(void);

ScoopThreadState *scoop_thread_current(void);
ScoopThreadState *scoop_thread_current_required(void);
void scoop_thread_require_managed(void);
void scoop_thread_require_single_managed_mutator(void);

#endif /* SCOOP_RT_THREAD_H */
