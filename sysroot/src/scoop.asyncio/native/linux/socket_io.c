#include "endpoint.h"
#include <errno.h>
#include <limits.h>
#include <sys/uio.h>

typedef enum { TCP_READ, TCP_WRITE, TCP_SHUTDOWN, UDP_READ, UDP_WRITE } SocketAction;
typedef struct {
    ScoopIoRequest request;
    ScoopIoEndpoint *endpoint;
    SocketAction action;
    void *bytes;
    int64_t length;
    struct sockaddr_storage address;
    struct msghdr message;
    struct iovec vector;
    struct ScoopDatagram *packet;
} SocketIo;

static bool writing(SocketAction action) {
    return action == TCP_WRITE || action == TCP_SHUTDOWN || action == UDP_WRITE;
}
static void finish(ScoopIoRequest *native) {
    SocketIo *request = (void *)native;
    if (native->completion.event.kind != SCOOP_IO_DATAGRAM)
        free(request->packet);
    scoop_io_endpoint_finished(native, request->endpoint, writing(request->action));
}
static void prepare(ScoopIoRequest *native, unsigned index, struct io_uring_sqe *sqe) {
    (void)index;
    SocketIo *request = (void *)native;
    int fd = request->endpoint->fd;
    unsigned prefix = request->length > INT_MAX ? INT_MAX : (unsigned)request->length;
    switch (request->action) {
    case TCP_READ:
        io_uring_prep_recv(sqe, fd, request->bytes, prefix, 0);
        break;
    case TCP_WRITE:
        io_uring_prep_send(sqe, fd, request->bytes, prefix, MSG_NOSIGNAL);
        break;
    case UDP_READ:
        io_uring_prep_recvmsg(sqe, fd, &request->message, 0);
        break;
    case UDP_WRITE:
        io_uring_prep_sendmsg(sqe, fd, &request->message, MSG_NOSIGNAL);
        break;
    case TCP_SHUTDOWN:
        abort(); // A short control operation never enters the ring.
    }
}
static void result(ScoopIoRequest *native, unsigned index, int status) {
    (void)index;
    SocketIo *request = (void *)native;
    ScoopIoEndpoint *endpoint = request->endpoint;
    ScoopIoEvent *event = &native->completion.event;
    if (status < 0) {
        if (request->action == TCP_READ && status != -ECANCELED)
            endpoint->read_error = -status;
        scoop_io_request_error(native, -status);
        return;
    }
    event->count = status;
    if (request->action == UDP_READ) {
        request->packet->source = scoop_socket_address_decode((struct sockaddr *)&request->address,
                                                              request->message.msg_namelen);
        if (!request->packet->source.family) {
            scoop_io_request_error(native, errno);
            return;
        }
        request->packet->count = status;
        request->packet->truncated = (request->message.msg_flags & MSG_TRUNC) != 0;
        event->kind = SCOOP_IO_DATAGRAM;
        event->resource = request->packet;
    } else if (request->action == TCP_READ && !status) {
        endpoint->eof = true;
        event->kind = SCOOP_IO_EOF;
    } else if (request->action != TCP_SHUTDOWN)
        event->kind = SCOOP_IO_BYTES;
    scoop_io_request_end(native);
}
static int submit_transfer(ScoopIoEndpoint *endpoint, int64_t id, SocketAction action, void *bytes,
                           int64_t length, struct ScoopSocketAddress address) {
    SocketIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    if (action == UDP_READ) {
        request->packet = calloc(1, sizeof(*request->packet));
        if (!request->packet) {
            free(request);
            return -ENOMEM;
        }
    }
    request->endpoint = endpoint;
    request->action = action;
    request->bytes = bytes;
    request->length = length;
    request->vector = (struct iovec){.iov_base = bytes, .iov_len = (size_t)length};
    request->message = (struct msghdr){.msg_name = &request->address,
                                       .msg_namelen = sizeof(request->address),
                                       .msg_iov = &request->vector,
                                       .msg_iovlen = 1};
    if (action == UDP_WRITE) {
        request->message.msg_namelen = scoop_socket_address_encode(address, &request->address);
        if (!request->message.msg_namelen) {
            int error = errno;
            free(request);
            return -error;
        }
    }
    request->request.prepare = prepare;
    request->request.result = result;
    request->request.finish = finish;
    scoop_io_request_init(&request->request, endpoint->loop, id);
    endpoint->active++;
    if (writing(action))
        endpoint->output = &request->request;
    else
        endpoint->input = &request->request;
    if (action == TCP_SHUTDOWN) {
        int status = shutdown(endpoint->fd, SHUT_WR);
        result(&request->request, 0, status < 0 ? -errno : 0);
    } else if (action == TCP_READ && (endpoint->eof || endpoint->read_error)) {
        result(&request->request, 0, -endpoint->read_error);
    } else
        scoop_io_request_queue(&request->request, 0);
    return 0;
}

int scoop_io_tcp_read(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count) {
    return submit_transfer(endpoint, id, TCP_READ, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_tcp_write(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count) {
    return submit_transfer(endpoint, id, TCP_WRITE, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_tcp_shutdown(ScoopIoEndpoint *endpoint, int64_t id) {
    return submit_transfer(endpoint, id, TCP_SHUTDOWN, NULL, 0, (struct ScoopSocketAddress){0});
}
int scoop_io_udp_receive(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count) {
    return submit_transfer(endpoint, id, UDP_READ, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_udp_send(ScoopIoEndpoint *endpoint, int64_t id, void *bytes, int64_t count,
                      struct ScoopSocketAddress address) {
    return submit_transfer(endpoint, id, UDP_WRITE, bytes, count, address);
}
void scoop_io_read_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
void scoop_io_tcp_write_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
void scoop_io_udp_receive_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
void scoop_io_udp_send_cancel(ScoopIoEndpoint *endpoint, int64_t id) {
    scoop_io_endpoint_cancel(endpoint, id);
}
struct ScoopDatagram scoop_io_udp_take_packet(struct ScoopDatagram *packet) {
    struct ScoopDatagram value = *packet;
    free(packet);
    return value;
}
