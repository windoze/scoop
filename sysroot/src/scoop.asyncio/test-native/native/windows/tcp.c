#include "socket_test.h"

ScoopIoEvent scoop_test_iocp_socket_next(ScoopIoLoop *loop) {
    ScoopIoEvent event;
    if (test_result(loop, &event))
        return (ScoopIoEvent){.kind = SCOOP_IO_POSIX_ERROR, .code = EIO};
    return event;
}

int scoop_test_iocp_socket_drain(ScoopIoLoop *loop) { return test_drain(loop); }

int scoop_test_iocp_tcp(bool ipv6) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoSocket *listener = NULL, *client = NULL, *server = NULL;
    int status = test_pair(loop, ipv6, &listener, &client, &server);
    CHECK(!status);
    unsigned char left = 0, right = 0, a = 0x80, b = 0xff;
    CHECK(!scoop_io_tcp_read(server, 5, &left, 1));
    scoop_io_read_cancel(server, 5);
    CHECK(server->active == 1 && loop->requests);
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == 5 && event.kind == SCOOP_IO_WINSOCK_ERROR &&
          event.code == WSA_OPERATION_ABORTED);
    CHECK(!server->read_error.code);
    CHECK(!scoop_io_tcp_read(server, 6, &left, 1));
    CHECK(!scoop_io_tcp_read(client, 7, &right, 1));
    CHECK(!scoop_io_tcp_write(client, 8, &a, 1));
    CHECK(!scoop_io_tcp_write(server, 9, &b, 1));
    unsigned seen = 0;
    for (unsigned i = 0; i < 4; i++) {
        CHECK(!test_result(loop, &event));
        CHECK(event.id >= 6 && event.id <= 9 && event.kind == SCOOP_IO_BYTES && event.count == 1);
        unsigned bit = 1u << (unsigned)(event.id - 6);
        CHECK(!(seen & bit));
        seen |= bit;
    }
    CHECK(left == a && right == b);
    CHECK(!scoop_io_tcp_shutdown(client, 10));
    CHECK(!test_result(loop, &event) && event.id == 10 && event.kind == SCOOP_IO_DONE);
    CHECK(!scoop_io_tcp_read(server, 11, &left, 1));
    CHECK(!test_result(loop, &event) && event.id == 11 && event.kind == SCOOP_IO_EOF);
    CHECK(!scoop_io_tcp_read(server, 12, &left, 1));
    CHECK(!test_result(loop, &event) && event.id == 12 && event.kind == SCOOP_IO_EOF);
    CHECK(!test_closed(client, 13) && !test_closed(server, 14) && !test_closed(listener, 15));
    CHECK(!test_drain(loop));
    return 0;
}

int scoop_test_iocp_socket_close(void) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoSocket *listener = NULL, *client = NULL, *server = NULL;
    CHECK(!test_pair(loop, false, &listener, &client, &server));
    int capacity = 4096;
    CHECK(setsockopt(client->socket, SOL_SOCKET, SO_SNDBUF, (const char *)&capacity,
                     sizeof(capacity)) == 0);
    CHECK(setsockopt(server->socket, SOL_SOCKET, SO_RCVBUF, (const char *)&capacity,
                     sizeof(capacity)) == 0);
    const unsigned length = 4 * 1024 * 1024;
    void *output = calloc(1, length);
    CHECK(output);
    unsigned char input = 0;
    CHECK(!scoop_io_tcp_read(client, 20, &input, 1));
    CHECK(!scoop_io_tcp_write(client, 21, output, length));
    CHECK(client->active == 2);
    SOCKET saved = client->socket;
    CHECK(!scoop_io_tcp_close(client, 22));
    DWORD flags;
    CHECK(GetHandleInformation((HANDLE)saved, &flags));
    unsigned seen = 0;
    ScoopIoEvent event;
    for (unsigned i = 0; i < 3; i++) {
        CHECK(!test_result(loop, &event));
        CHECK(event.id >= 20 && event.id <= 22);
        unsigned bit = 1u << (unsigned)(event.id - 20);
        CHECK(!(seen & bit));
        seen |= bit;
        if (event.id == 22)
            CHECK(i == 2 && event.kind == SCOOP_IO_DONE);
        else {
            CHECK(event.kind == SCOOP_IO_BYTES ||
                  (event.kind == SCOOP_IO_WINSOCK_ERROR && event.code == WSA_OPERATION_ABORTED));
            CHECK(event.count >= 0 && event.count <= length);
        }
    }
    CHECK(!GetHandleInformation((HANDLE)saved, &flags));
    free(output);
    CHECK(!test_closed(server, 23) && !test_closed(listener, 24));
    CHECK(!test_drain(loop));
    return 0;
}

ScoopIoSocketName scoop_test_iocp_socket_name(bool ipv6) {
    ScoopIoCreated created = scoop_io_loop_create();
    if (created.status.code)
        return (ScoopIoSocketName){.status = created.status};
    ScoopIoLoop *loop = created.pointer;
    ScoopIoSocket *socket = NULL;
    ScoopIoSocketName result = {0};
    int status = scoop_io_udp_bind(loop, 1, test_loopback(ipv6), false);
    if (!status)
        status = test_created(loop, &socket);
    if (!status) {
        result = scoop_io_udp_address(socket);
        status = test_closed(socket, 2);
    }
    if (test_drain(loop) || status)
        result.status = scoop_io_errno(-EIO);
    return result;
}
