#ifndef SCOOP_PLATFORM_SYNCHRONIZATION_H
#define SCOOP_PLATFORM_SYNCHRONIZATION_H

#include <errno.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>

struct ScoopPlatformMutex {
    pthread_mutex_t mutex;
    _Atomic uintptr_t owner;
};

static inline int sync_status(int error) {
    if (error == 0)
        return 0;
    errno = error;
    return -1;
}

static inline uintptr_t current_owner(void) { return (uintptr_t)pthread_self(); }

#endif
