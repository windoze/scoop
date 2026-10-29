#include "socket.h"
#include <errno.h>
#ifdef _MSC_VER
#include <MSWSock.h>
#else
#include <mswsock.h>
#endif

typedef enum { TCP_CONNECT, TCP_ACCEPT, TCP_LISTEN, UDP_BIND } SocketAction;
typedef struct {
    ScoopIoRequest request;
    ScoopIoSocket *socket, *listener;
    SocketAction action;
    struct sockaddr_storage address;
    unsigned char accepted_addresses[2 * (sizeof(struct sockaddr_storage) + 16)];
} SocketCreate;

static void finish(SocketCreate *request, ScoopIoStatus status) {
    ScoopIoSocket *socket = request->socket;
    if (!status.code && request->action == TCP_CONNECT &&
        setsockopt(socket->socket, SOL_SOCKET, SO_UPDATE_CONNECT_CONTEXT, NULL, 0) != 0)
        status = scoop_io_winsock(WSAGetLastError());
    if (!status.code && request->action == TCP_ACCEPT &&
        setsockopt(socket->socket, SOL_SOCKET, SO_UPDATE_ACCEPT_CONTEXT,
                   (const char *)&request->listener->socket, sizeof(SOCKET)) != 0)
        status = scoop_io_winsock(WSAGetLastError());
    ScoopIoEvent *event = &request->request.completion.event;
    if (status.code) {
        scoop_io_socket_error(event, status);
        if (socket->socket != INVALID_SOCKET)
            closesocket(socket->socket);
        free(socket);
    } else {
        event->kind = SCOOP_IO_RESOURCE;
        event->resource = socket;
    }
    if (request->listener)
        scoop_io_socket_finished(&request->request, request->listener, false);
    else {
        scoop_io_request_retire(&request->request);
        scoop_io_complete(request->request.loop, &request->request.completion, 0);
    }
}

static void packet(ScoopIoRequest *native, DWORD count, DWORD error) {
    (void)error;
    DWORD flags = 0;
    finish((void *)native, scoop_io_socket_result(native, &count, &flags));
}

static ScoopIoStatus connect_start(SocketCreate *request, struct ScoopSocketAddress target) {
    ScoopIoSocket *socket = request->socket;
    int length = scoop_socket_address_encode(target, &request->address);
    if (!length)
        return scoop_io_winsock(WSAGetLastError());
    ScoopIoStatus status = scoop_io_socket_create(socket);
    if (status.code)
        return status;
    request->request.handle = (HANDLE)socket->socket;
    struct sockaddr_storage local;
    struct ScoopSocketAddress any = {.family = target.family};
    int local_length = scoop_socket_address_encode(any, &local);
    if (bind(socket->socket, (struct sockaddr *)&local, local_length) != 0)
        return scoop_io_winsock(WSAGetLastError());
    status = scoop_io_socket_associate(socket);
    if (status.code)
        return status;
    GUID guid = WSAID_CONNECTEX;
    LPFN_CONNECTEX connect_ex;
    DWORD written;
    if (WSAIoctl(socket->socket, SIO_GET_EXTENSION_FUNCTION_POINTER, &guid, sizeof(guid),
                 &connect_ex, sizeof(connect_ex), &written, NULL, NULL) != 0)
        return scoop_io_winsock(WSAGetLastError());
    if (!connect_ex(socket->socket, (struct sockaddr *)&request->address, length, NULL, 0, NULL,
                    &request->request.overlapped)) {
        int error = WSAGetLastError();
        if (error != WSA_IO_PENDING)
            return scoop_io_winsock(error);
    }
    return (ScoopIoStatus){0};
}

static ScoopIoStatus accept_start(SocketCreate *request) {
    ScoopIoStatus status = scoop_io_socket_create(request->socket);
    if (status.code)
        return status;
    status = scoop_io_socket_associate(request->socket);
    if (status.code)
        return status;
    SOCKET listener = request->listener->socket;
    request->request.handle = (HANDLE)listener;
    GUID guid = WSAID_ACCEPTEX;
    LPFN_ACCEPTEX accept_ex;
    DWORD written;
    if (WSAIoctl(listener, SIO_GET_EXTENSION_FUNCTION_POINTER, &guid, sizeof(guid), &accept_ex,
                 sizeof(accept_ex), &written, NULL, NULL) != 0)
        return scoop_io_winsock(WSAGetLastError());
    DWORD address_length = sizeof(struct sockaddr_storage) + 16;
    if (!accept_ex(listener, request->socket->socket, request->accepted_addresses, 0,
                   address_length, address_length, &written, &request->request.overlapped)) {
        int error = WSAGetLastError();
        if (error != WSA_IO_PENDING)
            return scoop_io_winsock(error);
    }
    return (ScoopIoStatus){0};
}

static int submit(ScoopIoLoop *loop, int64_t id, SocketAction action,
                  struct ScoopSocketAddress address, int backlog, bool reuse,
                  ScoopIoSocket *listener) {
    SocketCreate *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->socket = scoop_io_socket_new(loop, listener ? listener->family : address.family);
    if (!request->socket) {
        free(request);
        return -ENOMEM;
    }
    request->action = action;
    request->listener = listener;
    scoop_io_request_init(&request->request, loop, id, INVALID_HANDLE_VALUE, packet);
    ScoopIoStatus status;
    if (listener) {
        listener->active++;
        listener->input = &request->request;
        status = accept_start(request);
    } else if (action == TCP_CONNECT)
        status = connect_start(request, address);
    else {
        struct ScoopSocketOpened opened =
            scoop_platform_socket_bind_owned(address, action == UDP_BIND, reuse, backlog);
        request->socket->socket = (SOCKET)opened.handle.value;
        status = opened.error ? scoop_io_winsock(opened.error)
                              : scoop_io_socket_associate(request->socket);
        finish(request, status);
        return 0;
    }
    if (status.code)
        finish(request, status);
    return 0;
}

int scoop_io_tcp_connect(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address) {
    return submit(loop, id, TCP_CONNECT, address, 0, false, NULL);
}
int scoop_io_tcp_listen(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                        int backlog, bool reuse) {
    return submit(loop, id, TCP_LISTEN, address, backlog, reuse, NULL);
}
int scoop_io_udp_bind(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                      bool reuse) {
    return submit(loop, id, UDP_BIND, address, 0, reuse, NULL);
}
int scoop_io_tcp_accept(ScoopIoSocket *socket, int64_t id) {
    return submit(socket->loop, id, TCP_ACCEPT, (struct ScoopSocketAddress){0}, 0, false, socket);
}
void scoop_io_tcp_connect_cancel(ScoopIoLoop *loop, int64_t id) {
    scoop_io_request_cancel(loop, id);
}
void scoop_io_accept_cancel(ScoopIoSocket *socket, int64_t id) {
    scoop_io_request_cancel(socket->loop, id);
}
