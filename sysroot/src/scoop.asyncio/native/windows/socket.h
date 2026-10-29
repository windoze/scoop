#ifndef SCOOP_ASYNCIO_WINDOWS_SOCKET_H
#define SCOOP_ASYNCIO_WINDOWS_SOCKET_H

#include "../common/socket.h"
#include "backend.h"

typedef struct ScoopIoSocket {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    SOCKET socket;
    int family;
    ScoopIoStatus read_error;
    unsigned active;
    bool closing, eof;
    ScoopIoRequest *input, *output;
} ScoopIoSocket;

static inline ScoopIoStatus scoop_io_winsock(int code) {
    return (ScoopIoStatus){SCOOP_IO_WINSOCK_ERROR, code};
}

ScoopIoSocket *scoop_io_socket_new(ScoopIoLoop *loop, int family);
ScoopIoStatus scoop_io_socket_create(ScoopIoSocket *socket);
ScoopIoStatus scoop_io_socket_associate(ScoopIoSocket *socket);
ScoopIoStatus scoop_io_socket_result(ScoopIoRequest *request, DWORD *count, DWORD *flags);
void scoop_io_socket_finished(ScoopIoRequest *request, ScoopIoSocket *socket, bool writing);
void scoop_io_socket_error(ScoopIoEvent *event, ScoopIoStatus status);

int scoop_io_tcp_connect(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address);
int scoop_io_tcp_listen(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                        int backlog, bool reuse);
int scoop_io_tcp_accept(ScoopIoSocket *socket, int64_t id);
void scoop_io_accept_cancel(ScoopIoSocket *socket, int64_t id);
void scoop_io_tcp_connect_cancel(ScoopIoLoop *loop, int64_t id);
ScoopIoSocketName scoop_io_tcp_address(ScoopIoSocket *socket, bool peer);
int scoop_io_tcp_read(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count);
int scoop_io_tcp_write(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count);
int scoop_io_tcp_shutdown(ScoopIoSocket *socket, int64_t id);
void scoop_io_read_cancel(ScoopIoSocket *socket, int64_t id);
int scoop_io_tcp_close(ScoopIoSocket *socket, int64_t id);
int scoop_io_udp_bind(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address, bool reuse);
ScoopIoSocketName scoop_io_udp_address(ScoopIoSocket *socket);
int scoop_io_udp_receive(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count);
int scoop_io_udp_send(ScoopIoSocket *socket, int64_t id, void *bytes, int64_t count,
                      struct ScoopSocketAddress address);
void scoop_io_udp_receive_cancel(ScoopIoSocket *socket, int64_t id);
int scoop_io_udp_close(ScoopIoSocket *socket, int64_t id);
struct ScoopDatagram scoop_io_udp_take_packet(struct ScoopDatagram *packet);

#endif
