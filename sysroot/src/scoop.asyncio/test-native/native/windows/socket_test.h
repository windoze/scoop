#ifndef SCOOP_ASYNCIO_WINDOWS_SOCKET_TEST_H
#define SCOOP_ASYNCIO_WINDOWS_SOCKET_TEST_H

#include "../../../native/windows/socket.h"
#include "loop_test.h"
#include <string.h>

static inline struct ScoopSocketAddress test_loopback(bool ipv6) {
    return ipv6 ? (struct ScoopSocketAddress){.family = 6, .d = 1}
                : (struct ScoopSocketAddress){.family = 4, .a = UINT32_C(0x7f000001)};
}

static inline int test_created(ScoopIoLoop *loop, ScoopIoSocket **socket) {
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_RESOURCE);
    *socket = event.resource;
    DWORD flags;
    CHECK(GetHandleInformation((HANDLE)(*socket)->socket, &flags) &&
          !(flags & HANDLE_FLAG_INHERIT));
    return 0;
}

static inline int test_closed(ScoopIoSocket *socket, int64_t id) {
    ScoopIoLoop *loop = socket->loop;
    CHECK(!scoop_io_tcp_close(socket, id));
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == id && event.kind == SCOOP_IO_DONE);
    return 0;
}

static inline int test_pair(ScoopIoLoop *loop, bool ipv6, ScoopIoSocket **listener,
                            ScoopIoSocket **client, ScoopIoSocket **server) {
    CHECK(!scoop_io_tcp_listen(loop, 1, test_loopback(ipv6), 8, false));
    CHECK(!test_created(loop, listener));
    ScoopIoSocketName address = scoop_io_tcp_address(*listener, false);
    CHECK(!address.status.code && address.address.port > 0);
    CHECK(!scoop_io_tcp_accept(*listener, 2));
    scoop_io_accept_cancel(*listener, 2);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == 2 && event.kind == SCOOP_IO_WINSOCK_ERROR &&
          event.code == WSA_OPERATION_ABORTED);
    CHECK(!scoop_io_tcp_accept(*listener, 3));
    CHECK(!scoop_io_tcp_connect(loop, 4, address.address));
    for (unsigned i = 0; i < 2; i++) {
        CHECK(!test_result(loop, &event) && event.kind == SCOOP_IO_RESOURCE);
        if (event.id == 3)
            *server = event.resource;
        else if (event.id == 4)
            *client = event.resource;
        else
            return __LINE__;
    }
    CHECK(*client && *server);
    ScoopIoSocketName peer = scoop_io_tcp_address(*client, true);
    CHECK(!peer.status.code && peer.address.family == address.address.family &&
          peer.address.port == address.address.port);
    peer = scoop_io_tcp_address(*server, true);
    address = scoop_io_tcp_address(*client, false);
    CHECK(!peer.status.code && !address.status.code && peer.address.port == address.address.port);
    DWORD flags;
    CHECK(GetHandleInformation((HANDLE)(*server)->socket, &flags) &&
          !(flags & HANDLE_FLAG_INHERIT));
    CHECK(GetHandleInformation((HANDLE)(*client)->socket, &flags) &&
          !(flags & HANDLE_FLAG_INHERIT));
    return 0;
}

#endif
