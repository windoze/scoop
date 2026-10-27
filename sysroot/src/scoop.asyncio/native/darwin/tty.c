#include "endpoint.h"
#include <errno.h>

typedef struct {
    ScoopIoWait wait;
    ScoopIoEndpoint *endpoint;
} TtyOpen;

static void open_finished(ScoopIoWait *wait) {
    TtyOpen *request = (void *)wait;
    if (wait->request.completion.event.kind != SCOOP_IO_RESOURCE) {
        if (request->endpoint->terminal)
            scoop_terminal_close(request->endpoint->terminal);
        free(request->endpoint);
    }
    scoop_io_request_finish(&wait->request);
}
static void open_cancel(ScoopIoRequest *request) { scoop_io_wait_end((void *)request); }

static void open_started(void *context) {
    TtyOpen *request = context;
    ScoopIoLoop *loop = request->wait.request.loop;
    ScoopIoEndpoint *endpoint = request->endpoint;
    if (!scoop_io_request_start(&request->wait.request))
        scoop_io_wait_end(&request->wait);
    else {
        endpoint->terminal = scoop_terminal_open();
        if (!endpoint->terminal)
            scoop_io_wait_error(&request->wait, errno);
        else {
            endpoint->fd = endpoint->terminal->fd;
            scoop_terminal_size size = scoop_terminal_get_size(endpoint->terminal);
            if (size.columns < 0)
                scoop_io_wait_error(&request->wait, errno);
            else {
                endpoint->columns = size.columns;
                endpoint->rows = size.rows;
                request->wait.request.completion.event.kind = SCOOP_IO_RESOURCE;
                request->wait.request.completion.event.resource = endpoint;
                scoop_io_wait_end(&request->wait);
            }
        }
    }
    scoop_io_action_end(loop);
}

int scoop_io_tty_open(ScoopIoLoop *loop, int64_t id) {
    TtyOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->endpoint = scoop_io_endpoint_new(loop);
    if (!request->endpoint) {
        free(request);
        return -ENOMEM;
    }
    request->wait.finish = open_finished;
    scoop_io_request_submit(&request->wait.request, loop, id, open_started, open_cancel);
    return 0;
}

int scoop_io_tty_set_raw(ScoopIoEndpoint *endpoint, bool enabled) {
    return scoop_terminal_set_raw(endpoint->terminal, enabled);
}
bool scoop_io_tty_is_raw(ScoopIoEndpoint *endpoint) {
    return scoop_terminal_raw_enabled(endpoint->terminal);
}
scoop_terminal_size scoop_io_tty_size(ScoopIoEndpoint *endpoint) {
    return scoop_terminal_get_size(endpoint->terminal);
}
