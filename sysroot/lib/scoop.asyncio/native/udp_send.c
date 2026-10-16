#include "sigpipe.h"
#include "udp.h"
#include <errno.h>

typedef struct {
    ScoopIoCompletion completion;
    uv_udp_send_t request;
    ScoopIoUdp *udp;
    int64_t count;
} ScoopIoUdpSend;

static void packet_sent(uv_udp_send_t *native, int status) {
    ScoopIoUdpSend *request = native->data;
    request->completion.event.kind = SCOOP_IO_BYTES;
    request->completion.event.count = status == 0 ? request->count : 0;
    scoop_io_complete(request->udp->loop, &request->completion, status);
}

int scoop_io_udp_send(ScoopIoUdp *udp, int64_t id, void *bytes, int64_t count,
                      struct ScoopSocketAddress address) {
    struct sockaddr_storage destination;
    if (!scoop_socket_address_encode(address, &destination))
        return uv_translate_sys_error(errno);
    ScoopIoUdpSend *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    request->request.data = request;
    request->udp = udp;
    request->count = count;
    /* Preserve the complete datagram length; never narrow it to unsigned int. */
    uv_buf_t buffer = {.base = bytes, .len = (size_t)count};
    ScoopIoSignalGuard guard;
    scoop_io_signal_enter(&guard);
    int status = uv_udp_send(&request->request, &udp->handle, &buffer, 1,
                             (struct sockaddr *)&destination, packet_sent);
    scoop_io_signal_leave(&guard);
    if (status < 0)
        free(request);
    return status;
}
