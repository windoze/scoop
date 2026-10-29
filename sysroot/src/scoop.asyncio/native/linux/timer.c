#include "../common/deadline.h"
#include "backend.h"
#include <errno.h>

typedef struct {
    ScoopIoRequest request;
    ScoopIoDeadline deadline;
    struct __kernel_timespec timeout;
} Timer;

static void prepare(ScoopIoRequest *native, unsigned index, struct io_uring_sqe *sqe) {
    (void)index;
    Timer *timer = (void *)native;
    int64_t remaining = scoop_io_remaining(timer->deadline);
    timer->timeout.tv_sec = remaining / 1000000000;
    timer->timeout.tv_nsec = remaining % 1000000000;
    io_uring_prep_timeout(sqe, &timer->timeout, 0, 0);
}
static void result(ScoopIoRequest *native, unsigned index, int status) {
    (void)index;
    Timer *timer = (void *)native;
    if (native->ended)
        return;
    if (status == -ETIME) {
        if (scoop_io_remaining(timer->deadline))
            scoop_io_request_queue(native, 0);
        else
            scoop_io_request_end(native);
    } else if (status < 0)
        scoop_io_request_error(native, -status);
    else
        scoop_io_request_end(native);
}
static void finish(ScoopIoRequest *native) {
    scoop_io_complete(native->loop, &native->completion, 0);
}
ScoopIoStatus scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos) {
    Timer *timer = calloc(1, sizeof(*timer));
    if (!timer)
        return scoop_io_errno(-ENOMEM);
    timer->deadline = scoop_io_deadline(seconds, nanos);
    timer->request.prepare = prepare;
    timer->request.result = result;
    timer->request.finish = finish;
    scoop_io_request_init(&timer->request, loop, id);
    scoop_io_request_queue(&timer->request, 0);
    return scoop_io_errno(0);
}
void scoop_io_timer_cancel(ScoopIoLoop *loop, int64_t id) { scoop_io_request_cancel(loop, id); }
