#include "endpoint.h"
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>

ScoopIoEndpoint *scoop_io_endpoint_new(ScoopIoLoop *loop) {
    ScoopIoEndpoint *endpoint = calloc(1, sizeof(*endpoint));
    if (endpoint) {
        endpoint->loop = loop;
        endpoint->fd = -1;
    }
    return endpoint;
}

int scoop_io_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL);
    if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0)
        return -errno;
    return 0;
}

void scoop_io_endpoint_idle(ScoopIoEndpoint *endpoint) {
    if (!endpoint->closing || endpoint->active)
        return;
    int status =
        endpoint->terminal ? scoop_terminal_close(endpoint->terminal) : close(endpoint->fd);
    int error = status < 0 ? errno : 0;
    scoop_io_complete(endpoint->loop, &endpoint->completion, -error);
}

void scoop_io_endpoint_finished(ScoopIoWait *wait, ScoopIoEndpoint *endpoint, bool writing) {
    if (writing)
        endpoint->output = NULL;
    else
        endpoint->input = NULL;
    endpoint->active--;
    scoop_io_request_finish(&wait->request);
    scoop_io_endpoint_idle(endpoint);
}

void scoop_io_endpoint_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_request_cancel(endpoint->loop, id);
}

static void close_endpoint(void *context) {
    ScoopIoEndpoint *endpoint = context;
    ScoopIoLoop *loop = endpoint->loop;
    endpoint->closing = true;
    // Each wait may finish synchronously. Keep the resource until both are processed.
    endpoint->active++;
    if (endpoint->input)
        scoop_io_wait_end(endpoint->input);
    if (endpoint->output)
        scoop_io_wait_end(endpoint->output);
    endpoint->active--;
    scoop_io_endpoint_idle(endpoint);
    scoop_io_action_end(loop);
}

int scoop_io_endpoint_close(ScoopIoEndpoint *endpoint, int64_t id) {
    endpoint->completion.event.id = id;
    scoop_io_action_begin(endpoint->loop);
    dispatch_async_f(endpoint->loop->queue, endpoint, close_endpoint);
    return 0;
}

int scoop_io_tcp_close(ScoopIoEndpoint *endpoint, int64_t id) {
    return scoop_io_endpoint_close(endpoint, id);
}
int scoop_io_udp_close(ScoopIoEndpoint *endpoint, int64_t id) {
    return scoop_io_endpoint_close(endpoint, id);
}
int scoop_io_tty_close(ScoopIoEndpoint *endpoint, int64_t id) {
    return scoop_io_endpoint_close(endpoint, id);
}

ScoopIoSocketName scoop_io_tcp_address(ScoopIoEndpoint *endpoint, bool peer) {
    struct sockaddr_storage address;
    socklen_t length = sizeof(address);
    int status = peer ? getpeername(endpoint->fd, (struct sockaddr *)&address, &length)
                      : getsockname(endpoint->fd, (struct sockaddr *)&address, &length);
    int error = status < 0 ? errno : 0;
    ScoopIoSocketName result = {.status = -error};
    if (!error)
        result.address = scoop_socket_address_decode((struct sockaddr *)&address, length);
    return result;
}

ScoopIoSocketName scoop_io_udp_address(ScoopIoEndpoint *endpoint) {
    return scoop_io_tcp_address(endpoint, false);
}
