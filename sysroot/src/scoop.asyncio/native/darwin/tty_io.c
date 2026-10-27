#include "endpoint.h"
#include <errno.h>

typedef struct {
    ScoopIoWait wait;
    ScoopIoEndpoint *endpoint;
    void *bytes;
    int64_t count;
    ScoopIoDeadline deadline;
    bool writing, timed, immediate;
} TtyIo;

static void transfer_finished(ScoopIoWait *wait) {
    TtyIo *request = (void *)wait;
    scoop_io_endpoint_finished(wait, request->endpoint, request->writing);
}
static void transfer_cancel(ScoopIoRequest *request) { scoop_io_wait_end((void *)request); }

static bool failed(ScoopIoEvent *event, int error) {
    event->kind = SCOOP_IO_POSIX_ERROR;
    event->code = error;
    return true;
}

static bool try_receive(TtyIo *request) {
    ScoopIoEndpoint *endpoint = request->endpoint;
    ScoopIoEvent *event = &request->wait.request.completion.event;
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
    ScoopIoEvent *event = &request->wait.request.completion.event;
    if (count < 0)
        return errno == EAGAIN || errno == EWOULDBLOCK ? false : failed(event, errno);
    event->kind = SCOOP_IO_BYTES;
    event->count = count;
    return true;
}

static void transfer_ready(void *context) {
    TtyIo *request = context;
    if (request->wait.ended)
        return;
    if (request->writing ? try_write(request) : try_receive(request))
        scoop_io_wait_end(&request->wait);
}

static void timeout_ready(void *context) {
    TtyIo *request = context;
    if (request->wait.ended)
        return;
    int64_t remaining = scoop_io_remaining(request->deadline);
    if (remaining) {
        dispatch_source_set_timer(request->wait.sources[request->wait.count - 1],
                                  dispatch_time(DISPATCH_TIME_NOW, remaining),
                                  DISPATCH_TIME_FOREVER, 0);
        return;
    }
    request->wait.request.completion.event.kind = SCOOP_IO_TTY_TIMEOUT;
    scoop_io_wait_end(&request->wait);
}

static void transfer_started(void *context) {
    TtyIo *request = context;
    ScoopIoLoop *loop = request->wait.request.loop;
    ScoopIoEndpoint *endpoint = request->endpoint;
    endpoint->active++;
    if (request->writing)
        endpoint->output = &request->wait;
    else
        endpoint->input = &request->wait;
    if (!scoop_io_request_start(&request->wait.request))
        scoop_io_wait_end(&request->wait);
    else if (request->writing ? try_write(request) : try_receive(request))
        scoop_io_wait_end(&request->wait);
    else if (request->immediate) {
        request->wait.request.completion.event.kind = SCOOP_IO_TTY_TIMEOUT;
        scoop_io_wait_end(&request->wait);
    } else {
        int status =
            scoop_io_wait_fd(&request->wait, endpoint->fd, request->writing, transfer_ready);
        if (!status && !request->writing)
            status = scoop_io_wait_fd(&request->wait, endpoint->terminal->notify_read, false,
                                      transfer_ready);
        if (!status && request->timed)
            status = scoop_io_wait_timer(&request->wait, request->deadline, timeout_ready);
        if (status < 0)
            scoop_io_wait_error(&request->wait, -status);
    }
    scoop_io_action_end(loop);
}

static int submit_transfer(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count,
                           bool writing, int64_t seconds, int nanos) {
    TtyIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->wait.finish = transfer_finished;
    request->endpoint = endpoint;
    request->bytes = bytes;
    request->count = count;
    request->writing = writing;
    request->timed = seconds >= 0;
    request->immediate = seconds == 0 && nanos == 0;
    if (request->timed)
        request->deadline = scoop_io_deadline(seconds, nanos);
    scoop_io_request_submit(&request->wait.request, endpoint->loop, id, transfer_started,
                            transfer_cancel);
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
