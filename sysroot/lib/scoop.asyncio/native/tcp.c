#include "tcp.h"
#include <errno.h>
#include <sys/socket.h>

ScoopIoTcp *scoop_io_tcp_new(ScoopIoLoop *loop, int *status) {
    ScoopIoTcp *tcp = calloc(1, sizeof(*tcp));
    if (!tcp) {
        *status = UV_ENOMEM;
        return NULL;
    }
    *status = uv_tcp_init(&loop->uv, &tcp->handle);
    if (*status < 0) {
        free(tcp);
        return NULL;
    }
    tcp->loop = loop;
    tcp->handle.data = tcp;
    return tcp;
}

int scoop_io_tcp_no_sigpipe(ScoopIoTcp *tcp) {
#ifdef __APPLE__
    uv_os_fd_t fd;
    int status = uv_fileno((uv_handle_t *)&tcp->handle, &fd);
    if (status < 0)
        return status;
    int enabled = 1;
    if (setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled)) < 0)
        return uv_translate_sys_error(errno);
#else
    (void)tcp;
#endif
    return 0;
}

void scoop_io_tcp_closed(uv_handle_t *handle) {
    ScoopIoTcp *tcp = handle->data;
    if (tcp->creating) {
        scoop_io_connect_finished(tcp->creating);
        free(tcp);
    } else if (tcp->completion.event.id != 0) {
        scoop_io_complete(tcp->loop, &tcp->completion, 0);
    } else {
        free(tcp);
    }
}

int scoop_io_tcp_close(ScoopIoTcp *tcp, int64_t id) {
    tcp->completion.event.id = id;
    if (tcp->accept)
        scoop_io_accept_cancel(tcp, 0);
    if (tcp->read)
        scoop_io_read_cancel(tcp, 0);
    uv_close((uv_handle_t *)&tcp->handle, scoop_io_tcp_closed);
    return 0;
}

ScoopIoSocketName scoop_io_tcp_address(ScoopIoTcp *tcp, bool peer) {
    struct sockaddr_storage address;
    int length = sizeof(address);
    int status = peer ? uv_tcp_getpeername(&tcp->handle, (struct sockaddr *)&address, &length)
                      : uv_tcp_getsockname(&tcp->handle, (struct sockaddr *)&address, &length);
    ScoopIoSocketName result = {0};
    result.status = status;
    if (status == 0)
        result.address = scoop_socket_address_decode((struct sockaddr *)&address, length);
    return result;
}
