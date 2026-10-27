#include "endpoint.h"
#include <errno.h>
#include <unistd.h>

typedef enum { TCP_CONNECT, TCP_LISTEN, UDP_BIND, TCP_ACCEPT } SocketAction;
typedef struct {
    ScoopIoWait wait;
    ScoopIoEndpoint *endpoint, *listener;
    struct ScoopSocketAddress address;
    SocketAction action;
    int backlog;
    bool reuse;
} SocketCreate;

static void create_finish(ScoopIoWait *wait) {
    SocketCreate *request = (void *)wait;
    ScoopIoEndpoint *listener = request->listener;
    if (request->wait.request.completion.event.kind != SCOOP_IO_RESOURCE) {
        if (request->endpoint->fd >= 0)
            close(request->endpoint->fd);
        free(request->endpoint);
    }
    if (listener)
        scoop_io_endpoint_finished(wait, listener, false);
    else
        scoop_io_request_finish(&wait->request);
}

static void create_cancel(ScoopIoRequest *request) { scoop_io_wait_end((void *)request); }

static void created(SocketCreate *request) {
    request->wait.request.completion.event.kind = SCOOP_IO_RESOURCE;
    request->wait.request.completion.event.resource = request->endpoint;
    scoop_io_wait_end(&request->wait);
}

static void connected(void *context) {
    SocketCreate *request = context;
    if (request->wait.ended)
        return;
    int error = 0;
    socklen_t length = sizeof(error);
    if (getsockopt(request->endpoint->fd, SOL_SOCKET, SO_ERROR, &error, &length) < 0)
        error = errno;
    if (error)
        scoop_io_wait_error(&request->wait, error);
    else
        created(request);
}

static void accept_ready(void *context) {
    SocketCreate *request = context;
    if (request->wait.ended)
        return;
    int fd;
    do {
        fd = accept(request->listener->fd, NULL, NULL);
    } while (fd < 0 && errno == EINTR);
    if (fd < 0) {
        int error = errno;
        if (error != EAGAIN && error != EWOULDBLOCK)
            scoop_io_wait_error(&request->wait, error);
        else if (!request->wait.count) {
            int status =
                scoop_io_wait_fd(&request->wait, request->listener->fd, false, accept_ready);
            if (status < 0)
                scoop_io_wait_error(&request->wait, -status);
        }
        return;
    }
    // prepare consumes fd on failure.
    fd = scoop_socket_prepare(fd);
    request->endpoint->fd = fd;
    if (fd < 0) {
        scoop_io_wait_error(&request->wait, errno);
        return;
    }
    int status = scoop_io_nonblocking(fd);
    if (status < 0)
        scoop_io_wait_error(&request->wait, -status);
    else
        created(request);
}

static void start_connect(SocketCreate *request) {
    struct sockaddr_storage address;
    socklen_t length = scoop_socket_address_encode(request->address, &address);
    if (!length) {
        scoop_io_wait_error(&request->wait, errno);
        return;
    }
    int fd = socket(((struct sockaddr *)&address)->sa_family, SOCK_STREAM, 0);
    if (fd >= 0)
        fd = scoop_socket_prepare(fd);
    request->endpoint->fd = fd;
    if (fd < 0) {
        scoop_io_wait_error(&request->wait, errno);
        return;
    }
    int status = scoop_io_nonblocking(fd);
    if (status < 0) {
        scoop_io_wait_error(&request->wait, -status);
        return;
    }
    if (connect(fd, (struct sockaddr *)&address, length) == 0) {
        created(request);
        return;
    }
    int error = errno;
    if (error != EINPROGRESS && error != EINTR) {
        scoop_io_wait_error(&request->wait, error);
        return;
    }
    status = scoop_io_wait_fd(&request->wait, fd, true, connected);
    if (status < 0)
        scoop_io_wait_error(&request->wait, -status);
}

static void create_start(void *context) {
    SocketCreate *request = context;
    ScoopIoLoop *loop = request->wait.request.loop;
    if (request->listener) {
        request->listener->active++;
        request->listener->input = &request->wait;
    }
    if (!scoop_io_request_start(&request->wait.request))
        scoop_io_wait_end(&request->wait);
    else if (request->action == TCP_CONNECT)
        start_connect(request);
    else if (request->action == TCP_ACCEPT)
        accept_ready(request);
    else {
        int fd = scoop_platform_socket_bind(request->address, request->action == UDP_BIND,
                                            request->reuse, request->backlog);
        request->endpoint->fd = fd;
        int status = fd < 0 ? -errno : scoop_io_nonblocking(fd);
        if (status < 0)
            scoop_io_wait_error(&request->wait, -status);
        else
            created(request);
    }
    scoop_io_action_end(loop);
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
    request->wait.finish = create_finish;
    request->action = action;
    request->address = address;
    request->backlog = backlog;
    request->reuse = reuse;
    request->listener = listener;
    scoop_io_request_submit(&request->wait.request, loop, id, create_start, create_cancel);
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
