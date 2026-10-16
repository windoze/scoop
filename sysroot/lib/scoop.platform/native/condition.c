#define _GNU_SOURCE 1
#include "synchronization.h"
#include <stdbool.h>
#include <stdlib.h>
#include <time.h>

pthread_cond_t *scoop_platform_condition_create(void) {
    pthread_cond_t *condition = malloc(sizeof(*condition));
    if (condition == NULL)
        return NULL;
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
    return condition;
}

int scoop_platform_condition_wait(pthread_cond_t *condition, struct ScoopPlatformMutex *mutex,
                                  bool timed, int64_t seconds, int32_t nanoseconds) {
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != current_owner())
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
            return -1;
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
    return error == ETIMEDOUT ? 1 : sync_status(error);
}

int scoop_platform_condition_signal(pthread_cond_t *condition, bool all) {
    return sync_status(all ? pthread_cond_broadcast(condition) : pthread_cond_signal(condition));
}

int scoop_platform_condition_destroy(pthread_cond_t *condition) {
    int error = pthread_cond_destroy(condition);
    if (error == 0)
        free(condition);
    return sync_status(error);
}
