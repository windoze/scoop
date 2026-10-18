#include "tcp.h"

static void accept_pending(ScoopIoTcp *listener) {
    ScoopIoAccept *request = listener->accept;
    if (!request || !listener->incoming)
        return;
    listener->accept = NULL;
    if (listener->incoming_error < 0) {
        listener->incoming = false;
        scoop_io_complete(listener->loop, &request->completion, listener->incoming_error);
        return;
    }
    int status;
    ScoopIoTcp *tcp = scoop_io_tcp_new(listener->loop, &status);
    if (!tcp) {
        /* Keep the pending connection available to the next accept. */
        scoop_io_complete(listener->loop, &request->completion, status);
        return;
    }
    request->tcp = tcp;
    tcp->creating = request;
    request->next = listener->loop->connections;
    listener->loop->connections = request;
    listener->incoming = false;
    status = uv_accept((uv_stream_t *)&listener->handle, (uv_stream_t *)&tcp->handle);
    if (status == 0)
        status = scoop_io_tcp_no_sigpipe(tcp);
    if (status < 0) {
        scoop_io_connect_failed(request, SCOOP_IO_UV_ERROR, status);
        return;
    }
    tcp->creating = NULL;
    request->completion.event.kind = SCOOP_IO_RESOURCE;
    request->completion.event.resource = tcp;
    scoop_io_connect_finished(request);
}

void scoop_io_connection(uv_stream_t *server, int status) {
    ScoopIoTcp *tcp = server->data;
    tcp->incoming = true;
    tcp->incoming_error = status;
    accept_pending(tcp);
}

int scoop_io_tcp_accept(ScoopIoTcp *tcp, int64_t id) {
    ScoopIoAccept *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    tcp->accept = request;
    accept_pending(tcp);
    return 0;
}

void scoop_io_accept_cancel(ScoopIoTcp *tcp, int64_t id) {
    ScoopIoAccept *request = tcp->accept;
    if (request && (id == 0 || request->completion.event.id == id)) {
        tcp->accept = NULL;
        scoop_io_complete(tcp->loop, &request->completion, UV_ECANCELED);
    } else if (id != 0) {
        scoop_io_tcp_connect_cancel(tcp->loop, id);
    }
}
