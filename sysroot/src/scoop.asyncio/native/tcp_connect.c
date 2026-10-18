#include "tcp.h"
#include <errno.h>
#include <unistd.h>

static ScoopIoConnect *new_connect(ScoopIoLoop *loop, int64_t id, int *status) {
    ScoopIoConnect *request = calloc(1, sizeof(*request));
    if (!request) {
        *status = UV_ENOMEM;
        return NULL;
    }
    request->tcp = scoop_io_tcp_new(loop, status);
    if (!request->tcp) {
        free(request);
        return NULL;
    }
    request->completion.event.id = id;
    request->tcp->creating = request;
    request->request.data = request;
    request->next = loop->connections;
    loop->connections = request;
    return request;
}

void scoop_io_connect_finished(ScoopIoConnect *request) {
    ScoopIoLoop *loop = request->tcp->loop;
    ScoopIoConnect **entry = &loop->connections;
    while (*entry != request)
        entry = &(*entry)->next;
    *entry = request->next;
    scoop_io_complete(loop, &request->completion, 0);
}

void scoop_io_connect_failed(ScoopIoConnect *request, int kind, int code) {
    request->completion.event.kind = kind;
    request->completion.event.code = code;
    if (!uv_is_closing((uv_handle_t *)&request->tcp->handle))
        uv_close((uv_handle_t *)&request->tcp->handle, scoop_io_tcp_closed);
}

static void connected(uv_connect_t *native, int status) {
    ScoopIoConnect *request = native->data;
    if (uv_is_closing((uv_handle_t *)&request->tcp->handle))
        return;
    if (status == 0)
        status = scoop_io_tcp_no_sigpipe(request->tcp);
    if (status < 0) {
        scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, status);
        return;
    }
    request->tcp->creating = NULL;
    request->completion.event.kind = SCOOP_IO_RESOURCE;
    request->completion.event.resource = request->tcp;
    scoop_io_connect_finished(request);
}

int scoop_io_tcp_connect(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address) {
    int status;
    ScoopIoConnect *request = new_connect(loop, id, &status);
    if (!request)
        return status;
    struct sockaddr_storage socket_address;
    if (!scoop_socket_address_encode(address, &socket_address)) {
        scoop_io_connect_failed(request, SCOOP_IO_POSIX_ERROR, errno);
        return 0;
    }
    status = uv_tcp_connect(&request->request, &request->tcp->handle,
                            (struct sockaddr *)&socket_address, connected);
    if (status < 0)
        scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, status);
    return 0;
}

int scoop_io_tcp_listen(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                        int backlog, bool reuse) {
    int status;
    ScoopIoConnect *request = new_connect(loop, id, &status);
    if (!request)
        return status;
    int fd = scoop_platform_socket_bind(address, false, reuse, backlog);
    if (fd < 0) {
        scoop_io_connect_failed(request, SCOOP_IO_POSIX_ERROR, errno);
        return 0;
    }
    status = uv_tcp_open(&request->tcp->handle, fd);
    if (status < 0) {
        (void)close(fd);
        scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, status);
        return 0;
    }
    status = uv_listen((uv_stream_t *)&request->tcp->handle, backlog, scoop_io_connection);
    if (status < 0)
        scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, status);
    else {
        request->tcp->creating = NULL;
        request->completion.event.kind = SCOOP_IO_RESOURCE;
        request->completion.event.resource = request->tcp;
        scoop_io_connect_finished(request);
    }
    return 0;
}

void scoop_io_tcp_connect_cancel(ScoopIoLoop *loop, int64_t id) {
    for (ScoopIoConnect *request = loop->connections; request; request = request->next) {
        if (request->completion.event.id == id) {
            scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, UV_ECANCELED);
            return;
        }
    }
}
