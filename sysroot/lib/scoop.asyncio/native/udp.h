#ifndef SCOOP_ASYNCIO_UDP_H
#define SCOOP_ASYNCIO_UDP_H

#include "socket.h"

typedef struct ScoopIoUdpRead ScoopIoUdpRead;
typedef struct ScoopIoUdp {
    ScoopIoCompletion completion;
    uv_udp_t handle;
    ScoopIoLoop *loop;
    ScoopIoUdpRead *read;
    ScoopIoCompletion *creating;
} ScoopIoUdp;

int scoop_io_udp_bind(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address, bool reuse);
ScoopIoSocketName scoop_io_udp_address(ScoopIoUdp *udp);
int scoop_io_udp_close(ScoopIoUdp *udp, int64_t id);
int scoop_io_udp_receive(ScoopIoUdp *udp, int64_t id, void *bytes, int64_t count);
void scoop_io_udp_receive_cancel(ScoopIoUdp *udp, int64_t id);
struct ScoopDatagram scoop_io_udp_take_packet(struct ScoopDatagram *packet);
int scoop_io_udp_send(ScoopIoUdp *udp, int64_t id, void *bytes, int64_t count,
                      struct ScoopSocketAddress address);

#endif
