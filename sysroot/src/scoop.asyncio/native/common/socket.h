#ifndef SCOOP_ASYNCIO_COMMON_SOCKET_H
#define SCOOP_ASYNCIO_COMMON_SOCKET_H

#include "../../../scoop.platform/native/socket_native.h"
#include "../loop.h"

typedef struct {
    struct ScoopSocketAddress address;
    ScoopIoStatus status;
} ScoopIoSocketName;

#endif
