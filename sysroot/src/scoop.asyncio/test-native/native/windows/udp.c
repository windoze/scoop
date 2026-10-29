#include "socket_test.h"

static int sent(ScoopIoLoop *loop, int64_t id, int64_t count) {
    ScoopIoEvent event;
    CHECK(!test_result(loop, &event) && event.id == id && event.kind == SCOOP_IO_BYTES &&
          event.count == count);
    return 0;
}

int scoop_test_iocp_udp(bool prefill) {
    ScoopIoCreated created = scoop_io_loop_create();
    CHECK(!created.status.code);
    ScoopIoLoop *loop = created.pointer;
    ScoopIoSocket *sender = NULL, *receiver = NULL;
    CHECK(!scoop_io_udp_bind(loop, 1, test_loopback(false), false) && !test_created(loop, &sender));
    CHECK(!scoop_io_udp_bind(loop, 2, test_loopback(false), false) &&
          !test_created(loop, &receiver));
    ScoopIoSocketName source = scoop_io_udp_address(sender);
    ScoopIoSocketName target = scoop_io_udp_address(receiver);
    CHECK(!source.status.code && !target.status.code);
    unsigned char data[5] = {0, 0xff, 0x80, 4, 5};
    unsigned char bytes[7];
    memset(bytes, 0x5a, sizeof(bytes));
    if (prefill) {
        CHECK(!scoop_io_udp_send(sender, 3, data, sizeof(data), target.address));
        CHECK(!sent(loop, 3, sizeof(data)));
        WSAPOLLFD ready = {.fd = receiver->socket, .events = POLLRDNORM};
        CHECK(WSAPoll(&ready, 1, 5000) == 1);
    }
    CHECK(!scoop_io_udp_receive(receiver, 4, bytes, 3));
    if (!prefill)
        CHECK(!scoop_io_udp_send(sender, 3, data, sizeof(data), target.address));
    ScoopIoEvent event;
    struct ScoopDatagram packet = {0};
    for (unsigned i = 0; i < (prefill ? 1u : 2u); i++) {
        CHECK(!test_result(loop, &event));
        if (event.id == 4) {
            CHECK(event.kind == SCOOP_IO_DATAGRAM);
            packet = scoop_io_udp_take_packet(event.resource);
        } else
            CHECK(event.id == 3 && event.kind == SCOOP_IO_BYTES && event.count == sizeof(data));
    }
    CHECK(packet.source.port == source.address.port && packet.count == 3 && packet.truncated);
    CHECK(!memcmp(bytes, data, 3) && bytes[3] == 0x5a && bytes[6] == 0x5a);
    CHECK(!scoop_io_udp_receive(receiver, 5, bytes, sizeof(bytes)));
    scoop_io_udp_receive_cancel(receiver, 5);
    CHECK(!test_result(loop, &event) && event.id == 5 && event.kind == SCOOP_IO_WINSOCK_ERROR &&
          event.code == WSA_OPERATION_ABORTED);
    CHECK(!scoop_io_udp_send(sender, 6, NULL, 0, target.address) && !sent(loop, 6, 0));
    CHECK(!scoop_io_udp_receive(receiver, 7, bytes, sizeof(bytes)));
    CHECK(!test_result(loop, &event) && event.id == 7 && event.kind == SCOOP_IO_DATAGRAM);
    packet = scoop_io_udp_take_packet(event.resource);
    CHECK(packet.count == 0 && !packet.truncated && packet.source.port == source.address.port);
    void *oversized = calloc(1, 65536);
    CHECK(oversized && !scoop_io_udp_send(sender, 8, oversized, 65536, target.address));
    CHECK(!test_result(loop, &event) && event.id == 8 && event.kind == SCOOP_IO_WINSOCK_ERROR &&
          event.code == WSAEMSGSIZE && event.count == 0);
    free(oversized);
    CHECK(!scoop_io_udp_receive(receiver, 9, bytes, sizeof(bytes)));
    CHECK(!scoop_io_udp_close(receiver, 10));
    CHECK(!test_result(loop, &event) && event.id == 9 && event.kind == SCOOP_IO_WINSOCK_ERROR &&
          event.code == WSA_OPERATION_ABORTED);
    CHECK(!test_result(loop, &event) && event.id == 10 && event.kind == SCOOP_IO_DONE);
    CHECK(!test_closed(sender, 11) && !test_drain(loop));
    return 0;
}
