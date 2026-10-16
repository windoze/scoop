#define _GNU_SOURCE 1
#include "synchronization.h"
#include <stdbool.h>
#include <stdlib.h>

struct ScoopPlatformMutex *scoop_platform_mutex_create(void) {
    struct ScoopPlatformMutex *mutex = malloc(sizeof(*mutex));
    if (mutex == NULL)
        return NULL;
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
    atomic_init(&mutex->owner, 0);
    return mutex;
}

int scoop_platform_mutex_lock(struct ScoopPlatformMutex *mutex, bool attempt) {
    int error = attempt ? pthread_mutex_trylock(&mutex->mutex) : pthread_mutex_lock(&mutex->mutex);
    if (error == EBUSY && attempt)
        return 1;
    if (error != 0)
        return sync_status(error);
    atomic_store_explicit(&mutex->owner, current_owner(), memory_order_relaxed);
    return 0;
}

int scoop_platform_mutex_unlock(struct ScoopPlatformMutex *mutex) {
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != current_owner())
        return sync_status(EPERM);
    atomic_store_explicit(&mutex->owner, 0, memory_order_relaxed);
    return sync_status(pthread_mutex_unlock(&mutex->mutex));
}

int scoop_platform_mutex_destroy(struct ScoopPlatformMutex *mutex) {
    /* Destroying a locked pthread mutex is undefined, not a portable busy probe. */
    if (atomic_load_explicit(&mutex->owner, memory_order_relaxed) != 0)
        return sync_status(EBUSY);
    int error = pthread_mutex_destroy(&mutex->mutex);
    if (error == 0)
        free(mutex);
    return sync_status(error);
}
