#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "synchronization.h"
#include <stdbool.h>
#include <stdlib.h>

struct ScoopPlatformMutex *scoop_platform_mutex_create(void) {
    struct ScoopPlatformMutex *mutex = malloc(sizeof(*mutex));
    if (mutex == NULL)
        return NULL;
#ifdef _WIN32
    InitializeSRWLock(&mutex->mutex);
#else
    pthread_mutexattr_t attributes;
    int error = pthread_mutexattr_init(&attributes);
    if (error == 0) {
        error = pthread_mutexattr_settype(&attributes, PTHREAD_MUTEX_ERRORCHECK);
        if (error == 0)
            error = pthread_mutex_init(&mutex->mutex, &attributes);
        (void)pthread_mutexattr_destroy(&attributes);
    }
    if (error != 0) {
        free(mutex);
        errno = error;
        return NULL;
    }
#endif
    atomic_init(&mutex->owner, 0);
    return mutex;
}

struct ScoopPlatformSyncResult scoop_platform_mutex_lock(struct ScoopPlatformMutex *mutex,
                                                         bool attempt) {
#ifdef _WIN32
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) == current_owner())
        return attempt ? (struct ScoopPlatformSyncResult){1, 0}
                       : sync_status(ERROR_POSSIBLE_DEADLOCK);
    if (attempt) {
        if (!TryAcquireSRWLockExclusive(&mutex->mutex))
            return (struct ScoopPlatformSyncResult){1, 0};
    } else {
        AcquireSRWLockExclusive(&mutex->mutex);
    }
#else
    int error = attempt ? pthread_mutex_trylock(&mutex->mutex) : pthread_mutex_lock(&mutex->mutex);
    if (error == EBUSY && attempt)
        return (struct ScoopPlatformSyncResult){1, 0};
    if (error != 0)
        return sync_status(error);
#endif
    atomic_store_explicit(&mutex->owner, current_owner(), memory_order_relaxed);
    return sync_status(0);
}

struct ScoopPlatformSyncResult scoop_platform_mutex_unlock(struct ScoopPlatformMutex *mutex) {
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != current_owner())
#ifdef _WIN32
        return sync_status(ERROR_NOT_OWNER);
#else
        return sync_status(EPERM);
#endif
    atomic_store_explicit(&mutex->owner, 0, memory_order_relaxed);
#ifdef _WIN32
    ReleaseSRWLockExclusive(&mutex->mutex);
    return sync_status(0);
#else
    return sync_status(pthread_mutex_unlock(&mutex->mutex));
#endif
}

struct ScoopPlatformSyncResult scoop_platform_mutex_destroy(struct ScoopPlatformMutex *mutex) {
    /* Destroying a locked pthread mutex is undefined, not a portable busy probe. */
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != 0)
#ifdef _WIN32
        return sync_status(ERROR_BUSY);
    free(mutex);
    return sync_status(0);
#else
        return sync_status(EBUSY);
    int error = pthread_mutex_destroy(&mutex->mutex);
    if (error == 0)
        free(mutex);
    return sync_status(error);
#endif
}
