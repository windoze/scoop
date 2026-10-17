#include "tcp.h"
#include <limits.h>

struct ScoopIoRead {
    ScoopIoCompletion completion;
    ScoopIoTcp *tcp;
    uv_buf_t buffer;
};

static void allocate_read(uv_handle_t *handle, size_t suggested, uv_buf_t *buffer) {
    (void)suggested;
    ScoopIoTcp *tcp = handle->data;
    *buffer = tcp->read->buffer;
}

static void read_finished(uv_stream_t *stream, ssize_t count, const uv_buf_t *buffer) {
    (void)buffer;
    if (count == 0)
        return;
    ScoopIoTcp *tcp = stream->data;
    ScoopIoRead *request = tcp->read;
    uv_read_stop(stream);
    tcp->read = NULL;
    int status = 0;
    if (count == UV_EOF) {
        tcp->read_terminal = UV_EOF;
        request->completion.event.kind = SCOOP_IO_EOF;
    } else if (count < 0) {
        tcp->read_terminal = (int)count;
        status = (int)count;
    } else {
        request->completion.event.kind = SCOOP_IO_BYTES;
        request->completion.event.count = count;
    }
    scoop_io_complete(tcp->loop, &request->completion, status);
}

int scoop_io_tcp_read(ScoopIoTcp *tcp, int64_t id, void *bytes, int64_t count) {
    ScoopIoRead *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    request->tcp = tcp;
    if (tcp->read_terminal) {
        if (tcp->read_terminal == UV_EOF) {
            request->completion.event.kind = SCOOP_IO_EOF;
            scoop_io_complete(tcp->loop, &request->completion, 0);
        } else {
            scoop_io_complete(tcp->loop, &request->completion, tcp->read_terminal);
        }
        return 0;
    }
    request->buffer = uv_buf_init(bytes, (unsigned)(count > INT_MAX ? INT_MAX : count));
    tcp->read = request;
    int status = uv_read_start((uv_stream_t *)&tcp->handle, allocate_read, read_finished);
    if (status < 0) {
        tcp->read = NULL;
        free(request);
    }
    return status;
}

void scoop_io_read_cancel(ScoopIoTcp *tcp, int64_t id) {
    ScoopIoRead *request = tcp->read;
    if (request && (id == 0 || request->completion.event.id == id)) {
        uv_read_stop((uv_stream_t *)&tcp->handle);
        tcp->read = NULL;
        scoop_io_complete(tcp->loop, &request->completion, UV_ECANCELED);
    }
}
