#include "socket.h"
#include <errno.h>
#include <limits.h>

typedef enum { TCP_READ, TCP_WRITE, TCP_SHUTDOWN, UDP_READ, UDP_WRITE } SocketAction;
typedef struct {
    ScoopIoRequest request;
    ScoopIoSocket *socket;
    SocketAction action;
    WSABUF buffer;
    DWORD flags;
    struct sockaddr_storage address;
    int address_length;
    struct ScoopDatagram *datagram;
} SocketIo;

static bool writing(SocketAction action) {
    return action == TCP_WRITE || action == TCP_SHUTDOWN || action == UDP_WRITE;
}

static void finish(SocketIo *request, DWORD count, ScoopIoStatus status, DWORD flags) {
    ScoopIoEvent *event = &request->request.completion.event;
    ScoopIoSocket *socket = request->socket;
    bool truncated = request->action == UDP_READ && status.domain == SCOOP_IO_WINSOCK_ERROR &&
                     status.code == WSAEMSGSIZE;
    if (truncated) {
        count = request->buffer.len;
        status.code = 0;
    }
    event->count = count;
    if (!status.code && request->action == UDP_READ) {
        request->datagram->source = scoop_socket_address_decode(
            (struct sockaddr *)&request->address, request->address_length);
        if (!request->datagram->source.family)
            status = scoop_io_winsock(WSAGetLastError());
        else {
            request->datagram->count = count;
            request->datagram->truncated = truncated || (flags & MSG_PARTIAL) != 0;
            event->kind = SCOOP_IO_DATAGRAM;
            event->resource = request->datagram;
        }
    } else if (!status.code && request->action == TCP_READ && count == 0) {
        socket->eof = true;
        event->kind = SCOOP_IO_EOF;
    } else if (!status.code && request->action != TCP_SHUTDOWN)
        event->kind = SCOOP_IO_BYTES;
    if (status.code) {
        if (request->action == TCP_READ && status.code != WSA_OPERATION_ABORTED)
            socket->read_error = status;
        scoop_io_socket_error(event, status);
    }
    if (event->kind != SCOOP_IO_DATAGRAM)
        free(request->datagram);
    scoop_io_socket_finished(&request->request, socket, writing(request->action));
}

static void packet(ScoopIoRequest *native, DWORD count, DWORD error) {
    (void)error;
    DWORD flags = 0;
    ScoopIoStatus status = scoop_io_socket_result(native, &count, &flags);
    finish((void *)native, count, status, flags);
}

static int submit(ScoopIoSocket *socket, int64_t id, SocketAction action, void *bytes,
                  int64_t count, struct ScoopSocketAddress address) {
    SocketIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    if (action == UDP_READ) {
        request->datagram = calloc(1, sizeof(*request->datagram));
        if (!request->datagram) {
            free(request);
            return -ENOMEM;
        }
    }
    request->socket = socket;
    request->action = action;
    request->buffer.buf = bytes;
    request->buffer.len = count > INT_MAX ? INT_MAX : (ULONG)count;
    request->address_length = sizeof(request->address);
    scoop_io_request_init(&request->request, socket->loop, id, (HANDLE)socket->socket, packet);
    socket->active++;
    if (writing(action))
        socket->output = &request->request;
    else
        socket->input = &request->request;
    int status = 0;
    switch (action) {
    case TCP_READ:
        if (socket->eof || socket->read_error.code) {
            finish(request, 0, socket->read_error, 0);
            return 0;
        }
        status = WSARecv(socket->socket, &request->buffer, 1, NULL, &request->flags,
                         &request->request.overlapped, NULL);
        break;
    case TCP_WRITE:
        status = WSASend(socket->socket, &request->buffer, 1, NULL, 0, &request->request.overlapped,
                         NULL);
        break;
    case TCP_SHUTDOWN:
        status = shutdown(socket->socket, SD_SEND);
        finish(request, 0, scoop_io_winsock(status ? WSAGetLastError() : 0), 0);
        return 0;
    case UDP_READ:
        status = WSARecvFrom(socket->socket, &request->buffer, 1, NULL, &request->flags,
                             (struct sockaddr *)&request->address, &request->address_length,
                             &request->request.overlapped, NULL);
        break;
    case UDP_WRITE:
        if (count > INT_MAX) {
            finish(request, 0, scoop_io_winsock(WSAEMSGSIZE), 0);
            return 0;
        }
        request->address_length = scoop_socket_address_encode(address, &request->address);
        if (!request->address_length) {
            finish(request, 0, scoop_io_winsock(WSAGetLastError()), 0);
            return 0;
        }
        status = WSASendTo(socket->socket, &request->buffer, 1, NULL, 0,
                           (struct sockaddr *)&request->address, request->address_length,
                           &request->request.overlapped, NULL);
        break;
    }
    if (status != 0) {
        int error = WSAGetLastError();
        if (error != WSA_IO_PENDING)
            finish(request, 0, scoop_io_winsock(error), request->flags);
    }
    return 0;
}

int scoop_io_tcp_read(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count) {
    return submit(socket, id, TCP_READ, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_tcp_write(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count) {
    return submit(socket, id, TCP_WRITE, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_tcp_shutdown(ScoopIoSocket *socket, int64_t id) {
    return submit(socket, id, TCP_SHUTDOWN, NULL, 0, (struct ScoopSocketAddress){0});
}
int scoop_io_udp_receive(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count) {
    return submit(socket, id, UDP_READ, bytes, count, (struct ScoopSocketAddress){0});
}
int scoop_io_udp_send(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count,
                      struct ScoopSocketAddress address) {
    return submit(socket, id, UDP_WRITE, bytes, count, address);
}
void scoop_io_read_cancel(ScoopIoSocket *socket, int64_t id) {
    scoop_io_request_cancel(socket->loop, id);
}
void scoop_io_tcp_write_cancel(ScoopIoSocket *socket, int64_t id) {
    scoop_io_request_cancel(socket->loop, id);
}
void scoop_io_udp_receive_cancel(ScoopIoSocket *socket, int64_t id) {
    scoop_io_request_cancel(socket->loop, id);
}
void scoop_io_udp_send_cancel(ScoopIoSocket *socket, int64_t id) {
    scoop_io_request_cancel(socket->loop, id);
}
struct ScoopDatagram scoop_io_udp_take_packet(struct ScoopDatagram *packet) {
    struct ScoopDatagram value = *packet;
    free(packet);
    return value;
}
