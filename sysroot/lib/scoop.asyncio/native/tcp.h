#ifndef SCOOP_ASYNCIO_TCP_H
#define SCOOP_ASYNCIO_TCP_H

#include "../../scoop.platform/native/socket_native.h"
#include "loop.h"

typedef struct ScoopIoRead ScoopIoRead;
typedef struct ScoopIoWrite ScoopIoWrite;
typedef struct ScoopIoShutdown ScoopIoShutdown;
typedef ScoopIoConnect ScoopIoAccept;

typedef struct ScoopIoTcp {
    ScoopIoCompletion completion;
    uv_tcp_t handle;
    ScoopIoLoop *loop;
    ScoopIoRead *read;
    ScoopIoWrite *write;
    ScoopIoShutdown *shutdown;
    ScoopIoAccept *accept;
    ScoopIoConnect *creating;
    bool incoming;
    int incoming_error;
    int read_terminal;
} ScoopIoTcp;

struct ScoopIoConnect {
    ScoopIoCompletion completion;
    uv_connect_t request;
    ScoopIoTcp *tcp;
    ScoopIoConnect *next;
};

typedef struct ScoopIoSocketName {
    struct ScoopSocketAddress address;
    int status;
} ScoopIoSocketName;

ScoopIoTcp *scoop_io_tcp_new(ScoopIoLoop *loop, int *status);
int scoop_io_tcp_no_sigpipe(ScoopIoTcp *tcp);
void scoop_io_tcp_closed(uv_handle_t *handle);
void scoop_io_connect_finished(ScoopIoConnect *request);
void scoop_io_connect_failed(ScoopIoConnect *request, int kind, int code);
void scoop_io_connection(uv_stream_t *server, int status);
void scoop_io_accept_cancel(ScoopIoTcp *tcp, int64_t id);
void scoop_io_read_cancel(ScoopIoTcp *tcp, int64_t id);

int scoop_io_tcp_connect(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address);
int scoop_io_tcp_listen(ScoopIoLoop *loop, int64_t id, struct ScoopSocketAddress address,
                        int backlog, bool reuse);
void scoop_io_tcp_connect_cancel(ScoopIoLoop *loop, int64_t id);
int scoop_io_tcp_accept(ScoopIoTcp *tcp, int64_t id);
ScoopIoSocketName scoop_io_tcp_address(ScoopIoTcp *tcp, bool peer);
int scoop_io_tcp_close(ScoopIoTcp *tcp, int64_t id);
int scoop_io_tcp_read(ScoopIoTcp *tcp, int64_t id, void *bytes, int64_t count);
int scoop_io_tcp_write(ScoopIoTcp *tcp, int64_t id, void *bytes, int64_t count);
void scoop_io_tcp_write_cancel(ScoopIoTcp *tcp, int64_t id);
int scoop_io_tcp_shutdown(ScoopIoTcp *tcp, int64_t id);

#endif
