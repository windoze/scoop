#include "udp.h"
#include <errno.h>
#include <unistd.h>

static void udp_closed(uv_handle_t *handle) {
    ScoopIoUdp *udp = handle->data;
    if (udp->creating) {
        scoop_io_complete(udp->loop, udp->creating, 0);
        free(udp);
    } else {
        scoop_io_complete(udp->loop, &udp->completion, 0);
    }
}

static void bind_failed(ScoopIoUdp *udp, int kind, int code) {
    udp->creating->event.kind = kind;
    udp->creating->event.code = code;
    uv_close((uv_handle_t *)&udp->handle, udp_closed);
}

int scoop_io_udp_bind(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                      bool reuse) {
    ScoopIoUdp *udp = calloc(1, sizeof(*udp));
    ScoopIoCompletion *request = calloc(1, sizeof(*request));
    if (!udp || !request) {
        free(udp);
        free(request);
        return UV_ENOMEM;
    }
    int status = uv_udp_init(&loop->uv, &udp->handle);
    if (status < 0) {
        free(udp);
        free(request);
        return status;
    }
    udp->loop = loop;
    udp->handle.data = udp;
    udp->creating = request;
    request->event.id = id;
    int fd = scoop_platform_socket_bind(address, true, reuse, 0);
    if (fd < 0) {
        bind_failed(udp, SCOOP_IO_POSIX_ERROR, errno);
        return 0;
    }
    status = uv_udp_open_ex(&udp->handle, fd, 0);
    if (status < 0) {
        (void)close(fd);
        bind_failed(udp, SCOOP_IO_UV_ERROR, status);
        return 0;
    }
    udp->creating = NULL;
    request->event.kind = SCOOP_IO_RESOURCE;
    request->event.resource = udp;
    scoop_io_complete(loop, request, 0);
    return 0;
}

ScoopIoSocketName scoop_io_udp_address(ScoopIoUdp *udp) {
    struct sockaddr_storage address;
    int length = sizeof(address);
    ScoopIoSocketName result = {0};
    result.status = uv_udp_getsockname(&udp->handle, (struct sockaddr *)&address, &length);
    if (result.status == 0)
        result.address = scoop_socket_address_decode((struct sockaddr *)&address, length);
    return result;
}

int scoop_io_udp_close(ScoopIoUdp *udp, int64_t id) {
    udp->completion.event.id = id;
    scoop_io_udp_receive_cancel(udp, 0);
    uv_close((uv_handle_t *)&udp->handle, udp_closed);
    return 0;
}
