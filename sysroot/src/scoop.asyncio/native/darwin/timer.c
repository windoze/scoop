#include "wait.h"
#include <errno.h>

typedef struct {
    ScoopIoWait wait;
    ScoopIoDeadline deadline;
} ScoopIoTimer;

static void timer_finish(ScoopIoWait *wait) { scoop_io_request_finish(&wait->request); }
static void timer_cancel(ScoopIoRequest *request) { scoop_io_wait_end((void *)request); }

static void timer_ready(void *context) {
    ScoopIoTimer *timer = context;
    if (timer->wait.ended)
        return;
    int64_t remaining = scoop_io_remaining(timer->deadline);
    if (!remaining) {
        scoop_io_wait_end(&timer->wait);
        return;
    }
    dispatch_source_set_timer(timer->wait.sources[0], dispatch_time(DISPATCH_TIME_NOW, remaining),
                              DISPATCH_TIME_FOREVER, 0);
}

static void timer_start(void *context) {
    ScoopIoTimer *timer = context;
    ScoopIoLoop *loop = timer->wait.request.loop;
    if (!scoop_io_request_start(&timer->wait.request))
        scoop_io_wait_end(&timer->wait);
    else {
        int status = scoop_io_wait_timer(&timer->wait, timer->deadline, timer_ready);
        if (status < 0)
            scoop_io_wait_error(&timer->wait, -status);
    }
    scoop_io_action_end(loop);
}

ScoopIoStatus scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos) {
    ScoopIoTimer *timer = calloc(1, sizeof(*timer));
    if (!timer)
        return scoop_io_errno(-ENOMEM);
    timer->deadline = scoop_io_deadline(seconds, nanos);
    timer->wait.finish = timer_finish;
    scoop_io_request_submit(&timer->wait.request, loop, id, timer_start, timer_cancel);
    return scoop_io_errno(0);
}

void scoop_io_timer_cancel(ScoopIoLoop *loop, int64_t id) { scoop_io_request_cancel(loop, id); }
