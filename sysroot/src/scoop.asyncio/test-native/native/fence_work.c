/* Test-only gates occupy actual backend worker slots; production I/O is unchanged. */
#include "../../native/common/work.h"
#include "fence_work.h"
#include <errno.h>
#ifdef _WIN32
#include "../../../scoop.platform/native/windows_api.h"
#define fence_lock(fence) AcquireSRWLockExclusive(&(fence)->mutex)
#define fence_unlock(fence) ReleaseSRWLockExclusive(&(fence)->mutex)
#define fence_wait(fence) SleepConditionVariableSRW(&(fence)->changed, &(fence)->mutex, INFINITE, 0)
#define fence_notify(fence) WakeAllConditionVariable(&(fence)->changed)
#else
#include <pthread.h>
#define fence_lock(fence) pthread_mutex_lock(&(fence)->mutex)
#define fence_unlock(fence) pthread_mutex_unlock(&(fence)->mutex)
#define fence_wait(fence) pthread_cond_wait(&(fence)->changed, &(fence)->mutex)
#define fence_notify(fence) pthread_cond_broadcast(&(fence)->changed)
#endif

struct TestWorkFence {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
#ifdef _WIN32
    SRWLOCK mutex;
    CONDITION_VARIABLE changed;
#else
    pthread_mutex_t mutex;
    pthread_cond_t changed;
#endif
    unsigned count, started, finished;
    bool released;
    ScoopIoWork requests[];
};

/**
 * Returns the configured native worker count, for the active backend.
 */
unsigned scoop_test_pool_size(void) { return scoop_io_work_width(); }

static void fence_work(ScoopIoWork *request) {
    TestWorkFence *fence = (TestWorkFence *)request->completion.event.resource;
    fence_lock(fence);
    fence->started++;
    fence_notify(fence);
    while (!fence->released)
        fence_wait(fence);
    fence_unlock(fence);
}

static void fence_finished(ScoopIoWork *request, int status) {
    TestWorkFence *fence = (TestWorkFence *)request->completion.event.resource;
    if (status != 0)
        abort();
    scoop_io_work_retire(request);
    if (++fence->finished == fence->count) {
#ifndef _WIN32
        pthread_cond_destroy(&fence->changed);
        pthread_mutex_destroy(&fence->mutex);
#endif
        scoop_io_complete(fence->loop, &fence->completion, 0);
    }
}

/**
 * Queues gates occupying all but spare worker slots. Release the fence before awaiting its
 * loop completion.
 */
ScoopIoCreated scoop_test_fence_create(ScoopIoLoop *loop, int64_t id, unsigned spare) {
    unsigned pool = scoop_test_pool_size();
    if (spare >= pool)
        return (ScoopIoCreated){NULL, scoop_io_errno(-EINVAL)};
    unsigned count = pool - spare;
    TestWorkFence *fence = calloc(1, sizeof(*fence) + count * sizeof(ScoopIoWork));
    if (!fence)
        return (ScoopIoCreated){NULL, scoop_io_errno(-ENOMEM)};
#ifdef _WIN32
    InitializeSRWLock(&fence->mutex);
    InitializeConditionVariable(&fence->changed);
#else
    int error = pthread_mutex_init(&fence->mutex, NULL);
    if (error != 0) {
        free(fence);
        return (ScoopIoCreated){NULL, scoop_io_errno(-error)};
    }
    error = pthread_cond_init(&fence->changed, NULL);
    if (error != 0) {
        pthread_mutex_destroy(&fence->mutex);
        free(fence);
        return (ScoopIoCreated){NULL, scoop_io_errno(-error)};
    }
#endif
    fence->loop = loop;
    fence->completion.event.id = id;
    fence->count = count;
    for (unsigned index = 0; index < count; index++) {
        fence->requests[index].completion.event.resource = fence;
        if (scoop_io_work_submit(&fence->requests[index], loop, 0, fence_work, fence_finished) != 0)
            abort();
    }
    return (ScoopIoCreated){fence, scoop_io_errno(0)};
}

/**
 * Blocks until every queued gate has entered a worker; call outside the event-loop work
 * being exercised.
 */
void scoop_test_fence_wait(TestWorkFence *fence) {
    fence_lock(fence);
    while (fence->started != fence->count)
        fence_wait(fence);
    fence_unlock(fence);
}

/**
 * Unblocks all occupied worker slots so their completion callbacks can finish on the loop.
 */
void scoop_test_fence_release(TestWorkFence *fence) {
    fence_lock(fence);
    fence->released = true;
    fence_notify(fence);
    fence_unlock(fence);
}
