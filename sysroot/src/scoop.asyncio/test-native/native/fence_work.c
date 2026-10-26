/* Test-only gates occupy actual backend worker slots; production I/O is unchanged. */
#include "../../native/common/work.h"
#include <errno.h>
#include <pthread.h>

typedef struct {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    unsigned count, started, finished;
    bool released;
    ScoopIoWork requests[];
} TestWorkFence;

/**
 * Returns the configured native worker count, for the active backend.
 */
unsigned scoop_test_pool_size(void) { return scoop_io_work_width(); }

static void fence_work(ScoopIoWork *request) {
    TestWorkFence *fence = (TestWorkFence *)request->completion.event.resource;
    pthread_mutex_lock(&fence->mutex);
    fence->started++;
    pthread_cond_broadcast(&fence->changed);
    while (!fence->released)
        pthread_cond_wait(&fence->changed, &fence->mutex);
    pthread_mutex_unlock(&fence->mutex);
}

static void fence_finished(ScoopIoWork *request, int status) {
    TestWorkFence *fence = (TestWorkFence *)request->completion.event.resource;
    if (status != 0)
        abort();
    scoop_io_work_retire(request);
    if (++fence->finished == fence->count) {
        pthread_cond_destroy(&fence->changed);
        pthread_mutex_destroy(&fence->mutex);
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
        return (ScoopIoCreated){NULL, -EINVAL};
    unsigned count = pool - spare;
    TestWorkFence *fence = calloc(1, sizeof(*fence) + count * sizeof(ScoopIoWork));
    if (!fence)
        return (ScoopIoCreated){NULL, -ENOMEM};
    int error = pthread_mutex_init(&fence->mutex, NULL);
    if (error != 0) {
        free(fence);
        return (ScoopIoCreated){NULL, -error};
    }
    error = pthread_cond_init(&fence->changed, NULL);
    if (error != 0) {
        pthread_mutex_destroy(&fence->mutex);
        free(fence);
        return (ScoopIoCreated){NULL, -error};
    }
    fence->loop = loop;
    fence->completion.event.id = id;
    fence->count = count;
    for (unsigned index = 0; index < count; index++) {
        fence->requests[index].completion.event.resource = fence;
        if (scoop_io_work_submit(&fence->requests[index], loop, 0, fence_work, fence_finished) != 0)
            abort();
    }
    return (ScoopIoCreated){fence, 0};
}

/**
 * Blocks until every queued gate has entered a worker; call outside the event-loop work
 * being exercised.
 */
void scoop_test_fence_wait(TestWorkFence *fence) {
    pthread_mutex_lock(&fence->mutex);
    while (fence->started != fence->count)
        pthread_cond_wait(&fence->changed, &fence->mutex);
    pthread_mutex_unlock(&fence->mutex);
}

/**
 * Unblocks all occupied worker slots so their completion callbacks can finish on the loop.
 */
void scoop_test_fence_release(TestWorkFence *fence) {
    pthread_mutex_lock(&fence->mutex);
    fence->released = true;
    pthread_cond_broadcast(&fence->changed);
    pthread_mutex_unlock(&fence->mutex);
}
