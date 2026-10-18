#define _GNU_SOURCE 1
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*ScoopPlatformThreadBody)(void *);
struct ScoopPlatformThread {
    pthread_t thread;
    _Atomic unsigned owners;
    ScoopPlatformThreadBody body;
    void *context;
};

static void release_thread(struct ScoopPlatformThread *thread) {
    if (atomic_fetch_sub_explicit(&thread->owners, 1, memory_order_acq_rel) == 1)
        free(thread);
}

static void *run_thread(void *argument) {
    struct ScoopPlatformThread *thread = argument;
    (void)thread->body(thread->context);
    release_thread(thread);
    return NULL;
}

struct ScoopPlatformThread *scoop_platform_thread_start(ScoopPlatformThreadBody body,
                                                        void *context) {
    struct ScoopPlatformThread *thread = malloc(sizeof(*thread));
    if (thread == NULL)
        return NULL;
    atomic_init(&thread->owners, 2);
    thread->body = body;
    thread->context = context;
    int error = pthread_create(&thread->thread, NULL, run_thread, thread);
    if (error != 0) {
        free(thread);
        errno = error;
        return NULL;
    }
    return thread;
}

int scoop_platform_thread_join(struct ScoopPlatformThread *thread) {
    if (pthread_equal(thread->thread, pthread_self())) {
        errno = EDEADLK;
        return -1;
    }
    int error = pthread_join(thread->thread, NULL);
    if (error != 0) {
        errno = error;
        return -1;
    }
    release_thread(thread);
    return 0;
}

void scoop_platform_thread_abandon(struct ScoopPlatformThread *thread) {
    (void)pthread_detach(thread->thread);
    release_thread(thread);
}

uint64_t scoop_platform_current_thread_id(void) { return (uint64_t)(uintptr_t)pthread_self(); }
int scoop_platform_thread_yield(void) { return sched_yield(); }
