#include "endpoint.h"
#include "../common/tty.h"
#include <errno.h>

int scoop_io_tty_open(ScoopIoLoop *loop, int64_t id) {
    ScoopIoCompletion *completion = calloc(1, sizeof(*completion));
    if (!completion)
        return -ENOMEM;
    ScoopIoEndpoint *endpoint = scoop_io_endpoint_new(loop);
    if (!endpoint) {
        free(completion);
        return -ENOMEM;
    }
    completion->event.id = id;
    endpoint->terminal = scoop_terminal_open();
    int error = endpoint->terminal ? 0 : errno;
    if (!error) {
        endpoint->fd = endpoint->terminal->fd;
        scoop_terminal_size size = scoop_terminal_get_size(endpoint->terminal);
        if (size.columns < 0)
            error = errno;
        else {
            endpoint->columns = size.columns;
            endpoint->rows = size.rows;
        }
    }
    if (error) {
        if (endpoint->terminal)
            scoop_terminal_close(endpoint->terminal);
        free(endpoint);
    } else {
        completion->event.kind = SCOOP_IO_RESOURCE;
        completion->event.resource = endpoint;
    }
    scoop_io_complete(loop, completion, -error);
    return 0;
}
ScoopIoStatus scoop_io_tty_set_raw(ScoopIoEndpoint *endpoint, bool enabled) {
    return scoop_io_terminal_raw(endpoint->terminal, enabled);
}
bool scoop_io_tty_is_raw(ScoopIoEndpoint *endpoint) {
    return scoop_terminal_raw_enabled(endpoint->terminal);
}
ScoopIoTtySize scoop_io_tty_size(ScoopIoEndpoint *endpoint) {
    return scoop_io_terminal_size(endpoint->terminal);
}
