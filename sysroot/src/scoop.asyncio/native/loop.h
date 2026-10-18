#ifndef SCOOP_ASYNCIO_LOOP_H
#define SCOOP_ASYNCIO_LOOP_H

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <uv.h>

typedef struct ScoopIoEvent {
    int64_t id;
    int kind;
    int code;
    int64_t count;
    void *resource;
} ScoopIoEvent;

enum {
    SCOOP_IO_DONE,
    SCOOP_IO_BYTES,
    SCOOP_IO_EOF,
    SCOOP_IO_RESOURCE,
    SCOOP_IO_UV_ERROR,
    SCOOP_IO_POSIX_ERROR,
    SCOOP_IO_ADDRESSES,
    SCOOP_IO_ADDRESS_ERROR,
    SCOOP_IO_DATAGRAM,
    SCOOP_IO_METADATA,
    SCOOP_IO_POSITION,
    SCOOP_IO_TEMPORARY
};

typedef struct ScoopIoCompletion {
    ScoopIoEvent event;
    struct ScoopIoCompletion *next;
} ScoopIoCompletion;

typedef struct ScoopIoTimer ScoopIoTimer;
typedef struct ScoopIoConnect ScoopIoConnect;
typedef struct ScoopIoWork ScoopIoWork;

typedef struct ScoopIoLoop {
    uv_loop_t uv;
    uv_async_t wake;
    ScoopIoCompletion *head;
    ScoopIoCompletion *tail;
    ScoopIoTimer *timers;
    ScoopIoConnect *connections;
    ScoopIoWork *work;
} ScoopIoLoop;

typedef struct ScoopIoCreated {
    void *pointer;
    int status;
} ScoopIoCreated;

void scoop_io_complete(ScoopIoLoop *loop, ScoopIoCompletion *completion, int status);
ScoopIoCreated scoop_io_loop_create(void);
void scoop_io_loop_wake(ScoopIoLoop *loop);
void scoop_io_loop_poll(ScoopIoLoop *loop, bool wait);
ScoopIoEvent scoop_io_loop_next(ScoopIoLoop *loop);
int scoop_io_loop_destroy(ScoopIoLoop *loop);
int scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos);
void scoop_io_timer_cancel(ScoopIoLoop *loop, int64_t id);

#endif
