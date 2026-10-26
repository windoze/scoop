#ifndef SCOOP_ASYNCIO_DARWIN_WAIT_H
#define SCOOP_ASYNCIO_DARWIN_WAIT_H

#include "backend.h"
#include <time.h>

typedef struct {
    uint64_t seconds;
    unsigned nanos;
} ScoopIoDeadline;

typedef struct ScoopIoWait {
    ScoopIoRequest request;
    dispatch_source_t sources[3];
    unsigned count, retired;
    bool ended;
    void (*finish)(struct ScoopIoWait *);
} ScoopIoWait;

ScoopIoDeadline scoop_io_deadline(int64_t seconds, int nanos);
int64_t scoop_io_remaining(ScoopIoDeadline deadline);
int scoop_io_wait_fd(ScoopIoWait *wait, int fd, bool writing, dispatch_function_t ready);
int scoop_io_wait_timer(ScoopIoWait *wait, ScoopIoDeadline deadline, dispatch_function_t ready);
void scoop_io_wait_end(ScoopIoWait *wait);
void scoop_io_wait_error(ScoopIoWait *wait, int error);

#endif
