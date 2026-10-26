#include "uv_loop.h"

struct ScoopIoTimer {
    /* The completion is first so the queue can release the finished allocation.
     */
    ScoopIoCompletion completion;
    uv_timer_t handle;
    ScoopIoLoop *loop;
    ScoopIoTimer *next;
    uint64_t seconds;
    unsigned nanos;
    uint64_t sampled;
    int status;
};

static void timer_closed(uv_handle_t *handle) {
    ScoopIoTimer *timer = handle->data;
    ScoopIoTimer **entry = &timer->loop->timers;
    while (*entry != timer)
        entry = &(*entry)->next;
    *entry = timer->next;
    scoop_io_complete(timer->loop, &timer->completion, timer->status);
}

static void timer_end(ScoopIoTimer *timer, int status) {
    timer->status = status;
    uv_timer_stop(&timer->handle);
    uv_close((uv_handle_t *)&timer->handle, timer_closed);
}

static void timer_tick(uv_timer_t *handle);

static void timer_schedule(ScoopIoTimer *timer) {
    /* Chunk very long durations without overflowing a nanosecond deadline. */
    uint64_t millis = timer->seconds > 86400
                          ? 86400000
                          : timer->seconds * 1000 + (timer->nanos + 999999) / 1000000;
    uv_update_time(&timer->loop->uv);
    int status = uv_timer_start(&timer->handle, timer_tick, millis, 0);
    if (status < 0)
        timer_end(timer, status);
}

static void timer_tick(uv_timer_t *handle) {
    ScoopIoTimer *timer = handle->data;
    uint64_t now = uv_hrtime();
    uint64_t elapsed = now - timer->sampled;
    uint64_t seconds = elapsed / 1000000000;
    unsigned nanos = (unsigned)(elapsed % 1000000000);
    timer->sampled = now;
    if (seconds > timer->seconds || (seconds == timer->seconds && nanos >= timer->nanos)) {
        timer_end(timer, 0);
        return;
    }
    timer->seconds -= seconds;
    if (nanos > timer->nanos) {
        timer->seconds--;
        timer->nanos += 1000000000;
    }
    timer->nanos -= nanos;
    timer_schedule(timer);
}

int scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos) {
    ScoopIoTimer *timer = calloc(1, sizeof(*timer));
    if (!timer)
        return UV_ENOMEM;
    int status = uv_timer_init(&loop->uv, &timer->handle);
    if (status < 0) {
        free(timer);
        return status;
    }
    timer->completion.event.id = id;
    timer->loop = loop;
    timer->seconds = (uint64_t)seconds;
    timer->nanos = (unsigned)nanos;
    timer->sampled = uv_hrtime();
    timer->handle.data = timer;
    timer->next = loop->timers;
    loop->timers = timer;
    timer_schedule(timer);
    return 0;
}

void scoop_io_timer_cancel(ScoopIoLoop *loop, int64_t id) {
    for (ScoopIoTimer *timer = loop->timers; timer; timer = timer->next) {
        if (timer->completion.event.id == id) {
            if (!uv_is_closing((uv_handle_t *)&timer->handle))
                timer_end(timer, UV_ECANCELED);
            return;
        }
    }
}
