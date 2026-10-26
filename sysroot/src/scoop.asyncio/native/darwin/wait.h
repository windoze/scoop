#ifndef SCOOP_ASYNCIO_DARWIN_WAIT_H
#define SCOOP_ASYNCIO_DARWIN_WAIT_H

#include "../common/deadline.h"
#include "backend.h"

typedef struct ScoopIoWait {
    ScoopIoRequest request;
    dispatch_source_t sources[3];
    unsigned count, retired;
    bool ended;
    void (*finish)(struct ScoopIoWait *);
} ScoopIoWait;

int scoop_io_wait_fd(ScoopIoWait *wait, int fd, bool writing, dispatch_function_t ready);
int scoop_io_wait_timer(ScoopIoWait *wait, ScoopIoDeadline deadline, dispatch_function_t ready);
void scoop_io_wait_end(ScoopIoWait *wait);
void scoop_io_wait_error(ScoopIoWait *wait, int error);

#endif
