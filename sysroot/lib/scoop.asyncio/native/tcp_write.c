#include "sigpipe.h"
#include "tcp.h"
#include <limits.h>

struct ScoopIoWrite {
    ScoopIoCompletion completion;
    uv_write_t request;
    ScoopIoTcp *tcp;
    uv_buf_t buffer;
};

struct ScoopIoShutdown {
    ScoopIoCompletion completion;
    uv_shutdown_t request;
    ScoopIoTcp *tcp;
};

static void write_finished(uv_write_t *native, int status) {
    ScoopIoWrite *request = native->data;
    request->tcp->write = NULL;
    request->completion.event.kind = SCOOP_IO_BYTES;
    request->completion.event.count = (int64_t)uv_write_nwritten(native);
    scoop_io_complete(request->tcp->loop, &request->completion, status);
}

int scoop_io_tcp_write(ScoopIoTcp *tcp, int64_t id, void *bytes, int64_t count) {
    ScoopIoWrite *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    request->request.data = request;
    request->tcp = tcp;
    request->buffer = uv_buf_init(bytes, (unsigned)(count > INT_MAX ? INT_MAX : count));
    ScoopIoSignalGuard guard;
    scoop_io_signal_enter(&guard);
    int status = uv_try_write((uv_stream_t *)&tcp->handle, &request->buffer, 1);
    if (status >= 0) {
        request->completion.event.kind = SCOOP_IO_BYTES;
        request->completion.event.count = status;
        scoop_io_signal_leave(&guard);
        scoop_io_complete(tcp->loop, &request->completion, 0);
        return 0;
    }
    if (status == UV_EAGAIN) {
        tcp->write = request;
        status = uv_write(&request->request, (uv_stream_t *)&tcp->handle, &request->buffer, 1,
                          write_finished);
    }
    scoop_io_signal_leave(&guard);
    if (status < 0) {
        tcp->write = NULL;
        free(request);
    }
    return status;
}

void scoop_io_tcp_write_cancel(ScoopIoTcp *tcp, int64_t id) {
    if (tcp->write && tcp->write->completion.event.id == id)
        (void)uv_cancel((uv_req_t *)&tcp->write->request);
}

static void shutdown_finished(uv_shutdown_t *native, int status) {
    ScoopIoShutdown *request = native->data;
    request->tcp->shutdown = NULL;
    scoop_io_complete(request->tcp->loop, &request->completion, status);
}

int scoop_io_tcp_shutdown(ScoopIoTcp *tcp, int64_t id) {
    ScoopIoShutdown *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->tcp = tcp;
    request->completion.event.id = id;
    request->request.data = request;
    tcp->shutdown = request;
    int status = uv_shutdown(&request->request, (uv_stream_t *)&tcp->handle, shutdown_finished);
    if (status < 0) {
        tcp->shutdown = NULL;
        free(request);
    }
    return status;
}
