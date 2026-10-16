#include "udp.h"
#include <limits.h>
#include <netinet/in.h>

struct ScoopIoUdpRead {
    ScoopIoCompletion completion;
    uv_buf_t buffer;
    struct ScoopDatagram *packet;
};

static void allocate_packet(uv_handle_t *handle, size_t suggested, uv_buf_t *buffer) {
    (void)suggested;
    ScoopIoUdp *udp = handle->data;
    *buffer = udp->read->buffer;
}

static void packet_received(uv_udp_t *handle, ssize_t count, const uv_buf_t *buffer,
                            const struct sockaddr *address, unsigned flags) {
    (void)buffer;
    if (count == 0 && !address)
        return;
    ScoopIoUdp *udp = handle->data;
    ScoopIoUdpRead *request = udp->read;
    uv_udp_recv_stop(handle);
    udp->read = NULL;
    if (count < 0) {
        free(request->packet);
        scoop_io_complete(udp->loop, &request->completion, (int)count);
        return;
    }
    socklen_t length =
        address->sa_family == AF_INET ? sizeof(struct sockaddr_in) : sizeof(struct sockaddr_in6);
    request->packet->source = scoop_socket_address_decode(address, length);
    request->packet->count = count;
    request->packet->truncated = (flags & UV_UDP_PARTIAL) != 0;
    request->completion.event.kind = SCOOP_IO_DATAGRAM;
    request->completion.event.resource = request->packet;
    scoop_io_complete(udp->loop, &request->completion, 0);
}

int scoop_io_udp_receive(ScoopIoUdp *udp, int64_t id, void *bytes, int64_t count) {
    ScoopIoUdpRead *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->packet = calloc(1, sizeof(*request->packet));
    if (!request->packet) {
        free(request);
        return UV_ENOMEM;
    }
    request->completion.event.id = id;
    request->buffer = uv_buf_init(bytes, (unsigned)(count > INT_MAX ? INT_MAX : count));
    udp->read = request;
    int status = uv_udp_recv_start(&udp->handle, allocate_packet, packet_received);
    if (status < 0) {
        udp->read = NULL;
        free(request->packet);
        free(request);
    }
    return status;
}

void scoop_io_udp_receive_cancel(ScoopIoUdp *udp, int64_t id) {
    ScoopIoUdpRead *request = udp->read;
    if (request && (id == 0 || request->completion.event.id == id)) {
        uv_udp_recv_stop(&udp->handle);
        udp->read = NULL;
        free(request->packet);
        scoop_io_complete(udp->loop, &request->completion, UV_ECANCELED);
    }
}

struct ScoopDatagram scoop_io_udp_take_packet(struct ScoopDatagram *packet) {
    struct ScoopDatagram value = *packet;
    free(packet);
    return value;
}
