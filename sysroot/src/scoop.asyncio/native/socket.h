#ifndef SCOOP_ASYNCIO_SOCKET_H
#define SCOOP_ASYNCIO_SOCKET_H

#include "../../scoop.platform/native/socket_native.h"
#include "loop.h"

typedef struct ScoopIoSocketName {
    struct ScoopSocketAddress address;
    int status;
} ScoopIoSocketName;

#endif
