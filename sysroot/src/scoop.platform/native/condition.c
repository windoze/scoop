#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "synchronization.h"
#include <stdbool.h>
#include <stdlib.h>
#include <time.h>

ScoopPlatformCondition *scoop_platform_condition_create(void) {
    ScoopPlatformCondition *condition = malloc(sizeof(*condition));
    if (condition == NULL)
        return NULL;
#ifdef _WIN32
    InitializeConditionVariable(condition);
#else
    pthread_condattr_t attributes;
    int error = pthread_condattr_init(&attributes);
    if (error == 0) {
#ifndef __APPLE__
        error = pthread_condattr_setclock(&attributes, CLOCK_MONOTONIC);
#endif
        if (error == 0)
            error = pthread_cond_init(condition, &attributes);
        (void)pthread_condattr_destroy(&attributes);
    }
    if (error != 0) {
        free(condition);
        errno = error;
        return NULL;
    }
#endif
    return condition;
}

#ifdef _WIN32
static struct ScoopPlatformSyncResult windows_wait(ScoopPlatformCondition *condition,
                                                   struct ScoopPlatformMutex *mutex, bool timed,
                                                   int64_t seconds, int32_t nanoseconds) {
    uint64_t duration = 0;
    DWORD milliseconds = INFINITE;
    if (timed) {
        if (seconds < 0 || nanoseconds < 0 || nanoseconds >= 1000000000)
            return sync_status(ERROR_INVALID_PARAMETER);
        uint64_t fraction = ((uint64_t)nanoseconds + 999999) / 1000000;
        if ((uint64_t)seconds > (UINT64_MAX - fraction) / 1000)
            return sync_status(ERROR_ARITHMETIC_OVERFLOW);
        duration = (uint64_t)seconds * 1000 + fraction;
        milliseconds = duration >= INFINITE ? INFINITE - 1 : (DWORD)duration;
    }
    uint64_t started = timed ? GetTickCount64() : 0;
    atomic_store_explicit(&mutex->owner, 0, memory_order_relaxed);
    BOOL success = SleepConditionVariableSRW(condition, &mutex->mutex, milliseconds, 0);
    DWORD error = success ? 0 : GetLastError();
    atomic_store_explicit(&mutex->owner, current_owner(), memory_order_relaxed);
    if (timed && error == ERROR_TIMEOUT) {
        /* Recheck the caller's predicate after a partial wait, avoiding a lost signal. */
        return (struct ScoopPlatformSyncResult){GetTickCount64() - started >= duration ? 1 : 0, 0};
    }
    return sync_status((int32_t)error);
}
#endif

struct ScoopPlatformSyncResult scoop_platform_condition_wait(ScoopPlatformCondition *condition,
                                                             struct ScoopPlatformMutex *mutex,
                                                             bool timed, int64_t seconds,
                                                             int32_t nanoseconds) {
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != current_owner())
#ifdef _WIN32
        return sync_status(ERROR_NOT_OWNER);
    return windows_wait(condition, mutex, timed, seconds, nanoseconds);
#else
        return sync_status(EPERM);
    struct timespec timeout = {0, 0};
    if (timed) {
        if (seconds < 0 || nanoseconds < 0 || nanoseconds >= 1000000000)
            return sync_status(EINVAL);
        timeout.tv_sec = (time_t)seconds;
        timeout.tv_nsec = nanoseconds;
#ifndef __APPLE__
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now) < 0)
            return sync_status(errno);
        int64_t nanos = now.tv_nsec + (int64_t)nanoseconds;
        int64_t carry = nanos / 1000000000;
        if (seconds > INT64_MAX - now.tv_sec - carry)
            return sync_status(EOVERFLOW);
        timeout.tv_sec = (time_t)(now.tv_sec + seconds + carry);
        timeout.tv_nsec = (long)(nanos % 1000000000);
#endif
    }
    atomic_store_explicit(&mutex->owner, 0, memory_order_relaxed);
    int error;
    if (!timed) {
        error = pthread_cond_wait(condition, &mutex->mutex);
    } else {
#ifdef __APPLE__
        error = pthread_cond_timedwait_relative_np(condition, &mutex->mutex, &timeout);
#else
        error = pthread_cond_timedwait(condition, &mutex->mutex, &timeout);
#endif
    }
    atomic_store_explicit(&mutex->owner, current_owner(), memory_order_relaxed);
    return error == ETIMEDOUT ? (struct ScoopPlatformSyncResult){1, 0} : sync_status(error);
#endif
}

struct ScoopPlatformSyncResult scoop_platform_condition_signal(ScoopPlatformCondition *condition,
                                                               bool all) {
#ifdef _WIN32
    if (all)
        WakeAllConditionVariable(condition);
    else
        WakeConditionVariable(condition);
    return sync_status(0);
#else
    return sync_status(all ? pthread_cond_broadcast(condition) : pthread_cond_signal(condition));
#endif
}

struct ScoopPlatformSyncResult scoop_platform_condition_destroy(ScoopPlatformCondition *condition) {
#ifdef _WIN32
    free(condition);
    return sync_status(0);
#else
    int error = pthread_cond_destroy(condition);
    if (error == 0)
        free(condition);
    return sync_status(error);
#endif
}
