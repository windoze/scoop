#include "endpoint.h"
#include <errno.h>
#include <limits.h>
#include <sys/uio.h>

typedef enum { TCP_READ, TCP_WRITE, TCP_SHUTDOWN, UDP_READ, UDP_WRITE } SocketAction;
typedef struct {
    ScoopIoWait wait;
    ScoopIoEndpoint *endpoint;
    SocketAction action;
    void *bytes;
    int64_t length;
    struct ScoopSocketAddress address;
    struct ScoopDatagram *packet;
} SocketIo;

static bool writing(SocketAction action) {
    return action == TCP_WRITE || action == TCP_SHUTDOWN || action == UDP_WRITE;
}

static void transfer_finish(ScoopIoWait *wait) {
    SocketIo *request = (void *)wait;
    if (wait->request.completion.event.kind != SCOOP_IO_DATAGRAM)
        free(request->packet);
    scoop_io_endpoint_finished(wait, request->endpoint, writing(request->action));
}
static void transfer_cancel(ScoopIoRequest *request) { scoop_io_wait_end((void *)request); }

static ssize_t transfer_datagram(SocketIo *request) {
    struct sockaddr_storage address;
    struct iovec vector = {.iov_base = request->bytes, .iov_len = (size_t)request->length};
    struct msghdr message = {.msg_name = &address, .msg_iov = &vector, .msg_iovlen = 1};
    if (request->action == UDP_WRITE) {
        message.msg_namelen = scoop_socket_address_encode(request->address, &address);
        if (!message.msg_namelen)
            return -1;
        return sendmsg(request->endpoint->fd, &message, 0);
    }
    message.msg_namelen = sizeof(address);
    ssize_t count = recvmsg(request->endpoint->fd, &message, 0);
    if (count >= 0) {
        request->packet->source =
            scoop_socket_address_decode((struct sockaddr *)&address, message.msg_namelen);
        if (!request->packet->source.family)
            return -1;
        request->packet->count = count;
        request->packet->truncated = (message.msg_flags & MSG_TRUNC) != 0;
    }
    return count;
}

static void transfer_ready(void *context) {
    SocketIo *request = context;
    if (request->wait.ended)
        return;
    ScoopIoEndpoint *endpoint = request->endpoint;
    ScoopIoEvent *event = &request->wait.request.completion.event;
    if (request->action == TCP_READ && (endpoint->eof || endpoint->read_error)) {
        if (endpoint->read_error)
            scoop_io_wait_error(&request->wait, endpoint->read_error);
        else {
            event->kind = SCOOP_IO_EOF;
            scoop_io_wait_end(&request->wait);
        }
        return;
    }
    ssize_t count;
    size_t prefix = (size_t)(request->length > INT_MAX ? INT_MAX : request->length);
    do {
        switch (request->action) {
        case TCP_READ:
            count = recv(endpoint->fd, request->bytes, prefix, 0);
            break;
        case TCP_WRITE:
            count = send(endpoint->fd, request->bytes, prefix, 0);
            break;
        case TCP_SHUTDOWN:
            count = shutdown(endpoint->fd, SHUT_WR);
            break;
        case UDP_READ:
        case UDP_WRITE:
            count = transfer_datagram(request);
            break;
        }
    } while (count < 0 && errno == EINTR);
    if (count < 0) {
        int error = errno;
        if (error == EAGAIN || error == EWOULDBLOCK) {
            if (!request->wait.count) {
                int status = scoop_io_wait_fd(&request->wait, endpoint->fd,
                                              writing(request->action), transfer_ready);
                if (status < 0)
                    scoop_io_wait_error(&request->wait, -status);
            }
            return;
        }
        if (request->action == TCP_READ)
            endpoint->read_error = error;
        scoop_io_wait_error(&request->wait, error);
        return;
    }
    event->count = count;
    if (request->action == UDP_READ) {
        event->kind = SCOOP_IO_DATAGRAM;
        event->resource = request->packet;
    } else if (request->action == TCP_READ && !count) {
        endpoint->eof = true;
        event->kind = SCOOP_IO_EOF;
    } else if (request->action != TCP_SHUTDOWN)
        event->kind = SCOOP_IO_BYTES;
    scoop_io_wait_end(&request->wait);
}

static void transfer_start(void *context) {
    SocketIo *request = context;
    ScoopIoLoop *loop = request->wait.request.loop;
    request->endpoint->active++;
    if (writing(request->action))
        request->endpoint->output = &request->wait;
    else
        request->endpoint->input = &request->wait;
    if (!scoop_io_request_start(&request->wait.request))
        scoop_io_wait_end(&request->wait);
    else
        transfer_ready(request);
    scoop_io_action_end(loop);
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
    request->wait.finish = transfer_finish;
    request->endpoint = endpoint;
    request->action = action;
    request->bytes = bytes;
    request->length = length;
    request->address = address;
    scoop_io_request_submit(&request->wait.request, endpoint->loop, id, transfer_start,
                            transfer_cancel);
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
