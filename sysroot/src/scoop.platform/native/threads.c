#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "sync_result.h"
#include <errno.h>
#ifdef _WIN32
#include "windows_api.h"
#include <process.h>
#else
#include <pthread.h>
#include <sched.h>
#endif
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*ScoopPlatformThreadBody)(void *);
struct ScoopPlatformThread {
#ifdef _WIN32
    HANDLE thread;
    unsigned id;
#else
    pthread_t thread;
#endif
    _Atomic unsigned owners;
    ScoopPlatformThreadBody body;
    void *context;
};

static void release_thread(struct ScoopPlatformThread *thread) {
    if (atomic_fetch_sub_explicit(&thread->owners, 1, memory_order_acq_rel) == 1)
        free(thread);
}

#ifdef _WIN32
static unsigned __stdcall run_thread(void *argument) {
#else
static void *run_thread(void *argument) {
#endif
    struct ScoopPlatformThread *thread = argument;
    (void)thread->body(thread->context);
    release_thread(thread);
    return 0;
}

struct ScoopPlatformThread *scoop_platform_thread_start(ScoopPlatformThreadBody body,
                                                        void *context) {
    struct ScoopPlatformThread *thread = malloc(sizeof(*thread));
    if (thread == NULL)
        return NULL;
    atomic_init(&thread->owners, 2);
    thread->body = body;
    thread->context = context;
#ifdef _WIN32
    thread->thread = (HANDLE)_beginthreadex(NULL, 0, run_thread, thread, 0, &thread->id);
    int error = thread->thread == NULL ? errno : 0;
#else
    int error = pthread_create(&thread->thread, NULL, run_thread, thread);
#endif
    if (error != 0) {
        free(thread);
        errno = error;
        return NULL;
    }
    return thread;
}

struct ScoopPlatformSyncResult scoop_platform_thread_join(struct ScoopPlatformThread *thread) {
#ifdef _WIN32
    if (thread->id == GetCurrentThreadId()) {
        /* A completed thread remains joinable even if its id has been reused. */
        DWORD state = WaitForSingleObject(thread->thread, 0);
        if (state == WAIT_TIMEOUT)
            return sync_status(ERROR_POSSIBLE_DEADLOCK);
        if (state == WAIT_FAILED)
            return sync_status((int32_t)GetLastError());
    }
    if (WaitForSingleObject(thread->thread, INFINITE) != WAIT_OBJECT_0)
        return sync_status((int32_t)GetLastError());
    if (!CloseHandle(thread->thread))
        return sync_status((int32_t)GetLastError());
#else
    if (pthread_equal(thread->thread, pthread_self()) != 0) {
        return sync_status(EDEADLK);
    }
    int error = pthread_join(thread->thread, NULL);
    if (error != 0) {
        return sync_status(error);
    }
#endif
    release_thread(thread);
    return sync_status(0);
}

void scoop_platform_thread_abandon(struct ScoopPlatformThread *thread) {
#ifdef _WIN32
    (void)CloseHandle(thread->thread);
#else
    (void)pthread_detach(thread->thread);
#endif
    release_thread(thread);
}

uint64_t scoop_platform_current_thread_id(void) {
#ifdef _WIN32
    return GetCurrentThreadId();
#else
    return (uint64_t)(uintptr_t)pthread_self();
#endif
}
int scoop_platform_thread_yield(void) {
#ifdef _WIN32
    (void)SwitchToThread();
    return 0;
#else
    return sched_yield();
#endif
}
