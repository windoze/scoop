/* Test-only gates occupy actual libuv worker slots; production I/O is unchanged. */
#include "../../native/loop.h"
#include <pthread.h>

typedef struct {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    unsigned count, started, finished;
    bool released;
    uv_work_t requests[];
} TestWorkFence;

/**
 * Returns the configured libuv worker count, applying the pinned backend's default and
 * limits.
 */
unsigned scoop_test_pool_size(void) {
    /* Match the pinned libuv threadpool's environment selection. */
    char value[16];
    size_t length = sizeof(value);
    unsigned count =
        uv_os_getenv("UV_THREADPOOL_SIZE", value, &length) == 0 ? (unsigned)atoi(value) : 4;
    if (count == 0)
        count = 1;
    return count > 1024 ? 1024 : count;
}

static void fence_work(uv_work_t *request) {
    TestWorkFence *fence = request->data;
    pthread_mutex_lock(&fence->mutex);
    fence->started++;
    pthread_cond_broadcast(&fence->changed);
    while (!fence->released)
        pthread_cond_wait(&fence->changed, &fence->mutex);
    pthread_mutex_unlock(&fence->mutex);
}

static void fence_finished(uv_work_t *request, int status) {
    TestWorkFence *fence = request->data;
    if (status != 0)
        abort();
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
        return (ScoopIoCreated){NULL, UV_EINVAL};
    unsigned count = pool - spare;
    TestWorkFence *fence = calloc(1, sizeof(*fence) + count * sizeof(uv_work_t));
    if (!fence)
        return (ScoopIoCreated){NULL, UV_ENOMEM};
    int error = pthread_mutex_init(&fence->mutex, NULL);
    if (error != 0) {
        free(fence);
        return (ScoopIoCreated){NULL, uv_translate_sys_error(error)};
    }
    error = pthread_cond_init(&fence->changed, NULL);
    if (error != 0) {
        pthread_mutex_destroy(&fence->mutex);
        free(fence);
        return (ScoopIoCreated){NULL, uv_translate_sys_error(error)};
    }
    fence->loop = loop;
    fence->completion.event.id = id;
    fence->count = count;
    for (unsigned index = 0; index < count; index++) {
        fence->requests[index].data = fence;
        if (uv_queue_work(&loop->uv, &fence->requests[index], fence_work, fence_finished) != 0)
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
