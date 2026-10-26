#include "../common/deadline.h"
#include "endpoint.h"
#include <errno.h>
#include <poll.h>

typedef struct {
    ScoopIoRequest request;
    ScoopIoEndpoint *endpoint;
    void *bytes;
    int64_t count;
    ScoopIoDeadline deadline;
    struct __kernel_timespec timeout;
    bool writing, timed, immediate;
} TtyIo;

static void finish(ScoopIoRequest *native) {
    TtyIo *request = (void *)native;
    scoop_io_endpoint_finished(native, request->endpoint, request->writing);
}
static bool failed(ScoopIoEvent *event, int error) {
    event->kind = SCOOP_IO_POSIX_ERROR;
    event->code = error;
    return true;
}

static bool try_receive(TtyIo *request) {
    ScoopIoEndpoint *endpoint = request->endpoint;
    ScoopIoEvent *event = &request->request.completion.event;
    if (endpoint->eof) {
        event->kind = SCOOP_IO_EOF;
        return true;
    }
    int ready;
    do {
        ready = scoop_terminal_wait(endpoint->terminal, 0);
    } while (ready < 0 && errno == EINTR);
    if (ready < 0)
        return failed(event, errno);
    if (ready & 2) {
        scoop_terminal_size size = scoop_terminal_get_size(endpoint->terminal);
        if (size.columns < 0)
            return failed(event, errno);
        if (size.columns != endpoint->columns || size.rows != endpoint->rows) {
            endpoint->columns = size.columns;
            endpoint->rows = size.rows;
            event->kind = SCOOP_IO_TTY_RESIZE;
            event->count = (size.columns << 32) | size.rows;
            return true;
        }
    }
    if (!(ready & 5))
        return false;
    int64_t count =
        scoop_terminal_read(endpoint->terminal, request->bytes, request->count, ready & 4);
    if (count < 0)
        return errno == EINTR || errno == EAGAIN || errno == EWOULDBLOCK ? false
                                                                         : failed(event, errno);
    endpoint->eof = count == 0;
    event->kind = count ? SCOOP_IO_BYTES : SCOOP_IO_EOF;
    event->count = count;
    return true;
}

static bool try_write(TtyIo *request) {
    int64_t count =
        scoop_terminal_try_write(request->endpoint->terminal, request->bytes, request->count);
    ScoopIoEvent *event = &request->request.completion.event;
    if (count < 0)
        return errno == EAGAIN || errno == EWOULDBLOCK ? false : failed(event, errno);
    event->kind = SCOOP_IO_BYTES;
    event->count = count;
    return true;
}

static void prepare(ScoopIoRequest *native, unsigned index, struct io_uring_sqe *sqe) {
    TtyIo *request = (void *)native;
    if (index == 2) {
        int64_t remaining = scoop_io_remaining(request->deadline);
        request->timeout.tv_sec = remaining / 1000000000;
        request->timeout.tv_nsec = remaining % 1000000000;
        io_uring_prep_timeout(sqe, &request->timeout, 0, 0);
    } else {
        int fd = index ? request->endpoint->terminal->notify_read : request->endpoint->fd;
        io_uring_prep_poll_add(sqe, fd, request->writing ? POLLOUT : POLLIN);
    }
}
static void result(ScoopIoRequest *native, unsigned index, int status) {
    TtyIo *request = (void *)native;
    if (native->ended)
        return;
    if (index == 2) {
        if (status != -ETIME) {
            if (status < 0)
                scoop_io_request_error(native, -status);
            else
                scoop_io_request_queue(native, index);
        } else if (scoop_io_remaining(request->deadline))
            scoop_io_request_queue(native, index);
        else {
            native->completion.event.kind = SCOOP_IO_TTY_TIMEOUT;
            scoop_io_request_end(native);
        }
    } else if (status < 0)
        scoop_io_request_error(native, -status);
    else if (request->writing ? try_write(request) : try_receive(request))
        scoop_io_request_end(native);
    else
        scoop_io_request_queue(native, index);
}
static int submit_transfer(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count,
                           bool writing, int64_t seconds, int nanos) {
    TtyIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->endpoint = endpoint;
    request->bytes = bytes;
    request->count = count;
    request->writing = writing;
    request->timed = seconds >= 0;
    request->immediate = seconds == 0 && nanos == 0;
    if (request->timed)
        request->deadline = scoop_io_deadline(seconds, nanos);
    request->request.prepare = prepare;
    request->request.result = result;
    request->request.finish = finish;
    scoop_io_request_init(&request->request, endpoint->loop, id);
    endpoint->active++;
    if (writing)
        endpoint->output = &request->request;
    else
        endpoint->input = &request->request;
    if (writing ? try_write(request) : try_receive(request))
        scoop_io_request_end(&request->request);
    else if (request->immediate) {
        request->request.completion.event.kind = SCOOP_IO_TTY_TIMEOUT;
        scoop_io_request_end(&request->request);
    } else {
        scoop_io_request_queue(&request->request, 0);
        if (!writing)
            scoop_io_request_queue(&request->request, 1);
        if (request->timed)
            scoop_io_request_queue(&request->request, 2);
    }
    return 0;
}

int scoop_io_tty_receive(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count,
                         int64_t seconds, int nanos) {
    return submit_transfer(endpoint, id, bytes, count, false, seconds, nanos);
}
int scoop_io_tty_write(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count) {
    return submit_transfer(endpoint, id, bytes, count, true, -1, 0);
}
void scoop_io_tty_cancel_read(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
void scoop_io_tty_cancel_write(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
