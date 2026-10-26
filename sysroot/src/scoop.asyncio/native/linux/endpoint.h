#ifndef SCOOP_ASYNCIO_LINUX_ENDPOINT_H
#define SCOOP_ASYNCIO_LINUX_ENDPOINT_H

#include "../../../scoop.platform/native/socket_native.h"
#include "../../../scoop.platform/native/tty/terminal.h"
#include "backend.h"

typedef struct ScoopIoEndpoint {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    int fd, read_error;
    unsigned active;
    bool closing, eof;
    ScoopIoRequest *input, *output;
    scoop_terminal *terminal;
    int64_t columns, rows;
} ScoopIoEndpoint;

typedef struct {
    struct ScoopSocketAddress address;
    int status;
} ScoopIoSocketName;

int scoop_io_nonblocking(int fd);
void scoop_io_endpoint_idle(ScoopIoEndpoint *endpoint);
void scoop_io_endpoint_finished(ScoopIoRequest *request, ScoopIoEndpoint *endpoint, bool writing);
void scoop_io_endpoint_cancel(ScoopIoEndpoint *endpoint, int64_t id);
int scoop_io_endpoint_close(ScoopIoEndpoint *endpoint, int64_t id);
ScoopIoEndpoint *scoop_io_endpoint_new(ScoopIoLoop *loop);

#endif
