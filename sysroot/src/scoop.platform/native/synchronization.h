#ifndef SCOOP_PLATFORM_SYNCHRONIZATION_H
#define SCOOP_PLATFORM_SYNCHRONIZATION_H

#include <errno.h>
#include <stdatomic.h>
#include <stdint.h>
#include "sync_result.h"
#ifdef _WIN32
#include "windows_api.h"
typedef CONDITION_VARIABLE ScoopPlatformCondition;
#else
#include <pthread.h>
typedef pthread_cond_t ScoopPlatformCondition;
#endif

struct ScoopPlatformMutex {
#ifdef _WIN32
    SRWLOCK mutex;
#else
    pthread_mutex_t mutex;
#endif
    _Atomic uintptr_t owner;
};

static inline uintptr_t current_owner(void) {
#ifdef _WIN32
    return GetCurrentThreadId();
#else
    return (uintptr_t)pthread_self();
#endif
}

#endif
