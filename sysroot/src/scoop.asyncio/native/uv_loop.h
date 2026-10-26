#ifndef SCOOP_ASYNCIO_UV_LOOP_H
#define SCOOP_ASYNCIO_UV_LOOP_H

#include "loop.h"
#include <uv.h>

typedef struct ScoopIoTimer ScoopIoTimer;
typedef struct ScoopIoConnect ScoopIoConnect;
typedef struct ScoopIoWork ScoopIoWork;

struct ScoopIoLoop {
    uv_loop_t uv;
    uv_async_t wake;
    ScoopIoCompletion *head;
    ScoopIoCompletion *tail;
    int64_t completions;
    ScoopIoTimer *timers;
    ScoopIoConnect *connections;
    ScoopIoWork *work;
};

#endif
