#include "endpoint.h"
#include <errno.h>
#include <unistd.h>

typedef enum { TCP_CONNECT, TCP_LISTEN, UDP_BIND, TCP_ACCEPT } SocketAction;
typedef struct {
    ScoopIoRequest request;
    ScoopIoEndpoint *endpoint, *listener;
    struct sockaddr_storage address;
    socklen_t length;
    SocketAction action;
} SocketCreate;

static void create_finish(ScoopIoRequest *native) {
    SocketCreate *request = (void *)native;
    if (native->completion.event.kind != SCOOP_IO_RESOURCE) {
        if (request->endpoint->fd >= 0)
            close(request->endpoint->fd);
        free(request->endpoint);
    }
    if (request->listener)
        scoop_io_endpoint_finished(native, request->listener, false);
    else
        scoop_io_complete(native->loop, &native->completion, 0);
}
static void created(SocketCreate *request) {
    request->request.completion.event.kind = SCOOP_IO_RESOURCE;
    request->request.completion.event.resource = request->endpoint;
    scoop_io_request_end(&request->request);
}
static void prepare(ScoopIoRequest *native, unsigned index, struct io_uring_sqe *sqe) {
    (void)index;
    SocketCreate *request = (void *)native;
    if (request->action == TCP_ACCEPT)
        io_uring_prep_accept(sqe, request->listener->fd, NULL, NULL, SOCK_CLOEXEC | SOCK_NONBLOCK);
    else
        io_uring_prep_connect(sqe, request->endpoint->fd, (struct sockaddr *)&request->address,
                              request->length);
}
static void result(ScoopIoRequest *native, unsigned index, int status) {
    (void)index;
    SocketCreate *request = (void *)native;
    if (status < 0) {
        scoop_io_request_error(native, -status);
        return;
    }
    if (request->action == TCP_ACCEPT) {
        // prepare consumes an accepted descriptor on failure.
        request->endpoint->fd = scoop_socket_prepare(status);
        if (request->endpoint->fd < 0) {
            scoop_io_request_error(native, errno);
            return;
        }
    }
    // A late success still has an owner. The shared operation closes it before resuming
    // cancellation.
    created(request);
}
static int submit_create(ScoopIoLoop *loop, int64_t id, SocketAction action,
                         struct ScoopSocketAddress address, int backlog, bool reuse,
                         ScoopIoEndpoint *listener) {
    SocketCreate *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->endpoint = scoop_io_endpoint_new(loop);
    if (!request->endpoint) {
        free(request);
        return -ENOMEM;
    }
    request->action = action;
    request->listener = listener;
    request->request.prepare = prepare;
    request->request.result = result;
    request->request.finish = create_finish;
    scoop_io_request_init(&request->request, loop, id);
    if (listener) {
        listener->active++;
        listener->input = &request->request;
        scoop_io_request_queue(&request->request, 0);
    } else if (action == TCP_CONNECT) {
        request->length = scoop_socket_address_encode(address, &request->address);
        if (!request->length)
            scoop_io_request_error(&request->request, errno);
        else {
            int fd = socket(((struct sockaddr *)&request->address)->sa_family,
                            SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK, 0);
            if (fd >= 0)
                fd = scoop_socket_prepare(fd);
            request->endpoint->fd = fd;
            if (fd < 0)
                scoop_io_request_error(&request->request, errno);
            else
                scoop_io_request_queue(&request->request, 0);
        }
    } else {
        int fd = scoop_platform_socket_bind(address, action == UDP_BIND, reuse, backlog);
        request->endpoint->fd = fd;
        int status = fd < 0 ? -errno : scoop_io_nonblocking(fd);
        if (status < 0)
            scoop_io_request_error(&request->request, -status);
        else
            created(request);
    }
    return 0;
}

int scoop_io_tcp_connect(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address) {
    return submit_create(loop, id, TCP_CONNECT, address, 0, false, NULL);
}
int scoop_io_tcp_listen(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                        int backlog, bool reuse) {
    return submit_create(loop, id, TCP_LISTEN, address, backlog, reuse, NULL);
}
int scoop_io_udp_bind(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                      bool reuse) {
    return submit_create(loop, id, UDP_BIND, address, 0, reuse, NULL);
}
int scoop_io_tcp_accept(ScoopIoEndpoint *endpoint, int64_t id) {
    return submit_create(endpoint->loop, id, TCP_ACCEPT, (struct ScoopSocketAddress){0}, 0, false,
                         endpoint);
}
void scoop_io_tcp_connect_cancel(ScoopIoLoop *loop, int64_t id) {
    scoop_io_request_cancel(loop, id);
}
void scoop_io_accept_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
