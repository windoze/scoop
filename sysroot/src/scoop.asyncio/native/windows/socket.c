#include "socket.h"

ScoopIoSocket *scoop_io_socket_new(ScoopIoLoop *loop, int family) {
    ScoopIoSocket *socket = calloc(1, sizeof(*socket));
    if (socket) {
        socket->loop = loop;
        socket->family = family;
        socket->socket = INVALID_SOCKET;
    }
    return socket;
}

ScoopIoStatus scoop_io_socket_create(ScoopIoSocket *socket) {
    int error = scoop_socket_startup();
    if (error)
        return scoop_io_winsock(error);
    socket->socket = WSASocketW(socket->family == 4 ? AF_INET : AF_INET6, SOCK_STREAM, 0, NULL, 0,
                                WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT);
    return scoop_io_winsock(socket->socket == INVALID_SOCKET ? WSAGetLastError() : 0);
}

ScoopIoStatus scoop_io_socket_associate(ScoopIoSocket *socket) {
    HANDLE port = CreateIoCompletionPort((HANDLE)socket->socket, socket->loop->port, 0, 0);
    return scoop_io_win32(port ? 0 : GetLastError());
}

ScoopIoStatus scoop_io_socket_result(ScoopIoRequest *request, DWORD *count, DWORD *flags) {
    BOOL success =
        WSAGetOverlappedResult((SOCKET)request->handle, &request->overlapped, count, FALSE, flags);
    return scoop_io_winsock(success ? 0 : WSAGetLastError());
}

void scoop_io_socket_error(ScoopIoEvent *event, ScoopIoStatus status) {
    if (status.code) {
        event->kind = status.domain;
        event->code = status.code;
    }
}

static void idle(ScoopIoSocket *socket) {
    if (!socket->closing || socket->active)
        return;
    int error = closesocket(socket->socket) == 0 ? 0 : WSAGetLastError();
    scoop_io_socket_error(&socket->completion.event, scoop_io_winsock(error));
    scoop_io_complete(socket->loop, &socket->completion, 0);
}

void scoop_io_socket_finished(ScoopIoRequest *request, ScoopIoSocket *socket, bool writing) {
    scoop_io_request_retire(request);
    if (writing)
        socket->output = NULL;
    else
        socket->input = NULL;
    socket->active--;
    scoop_io_complete(request->loop, &request->completion, 0);
    idle(socket);
}

static int close_socket(ScoopIoSocket *socket, int64_t id) {
    socket->completion.event.id = id;
    socket->closing = true;
    if (socket->input)
        scoop_io_request_cancel(socket->loop, socket->input->completion.event.id);
    if (socket->output)
        scoop_io_request_cancel(socket->loop, socket->output->completion.event.id);
    idle(socket);
    return 0;
}

int scoop_io_tcp_close(ScoopIoSocket *socket, int64_t id) { return close_socket(socket, id); }
int scoop_io_udp_close(ScoopIoSocket *socket, int64_t id) { return close_socket(socket, id); }

ScoopIoSocketName scoop_io_tcp_address(ScoopIoSocket *socket, bool peer) {
    struct sockaddr_storage address;
    int length = sizeof(address);
    int status = peer ? getpeername(socket->socket, (struct sockaddr *)&address, &length)
                      : getsockname(socket->socket, (struct sockaddr *)&address, &length);
    ScoopIoSocketName result = {.status = scoop_io_winsock(status ? WSAGetLastError() : 0)};
    if (!result.status.code) {
        result.address = scoop_socket_address_decode((struct sockaddr *)&address, length);
        if (!result.address.family)
            result.status = scoop_io_winsock(WSAGetLastError());
    }
    return result;
}

ScoopIoSocketName scoop_io_udp_address(ScoopIoSocket *socket) {
    return scoop_io_tcp_address(socket, false);
}
