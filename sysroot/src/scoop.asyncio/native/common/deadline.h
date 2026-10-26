#ifndef SCOOP_ASYNCIO_DEADLINE_H
#define SCOOP_ASYNCIO_DEADLINE_H
#include <stdint.h>
#include <time.h>
typedef struct {
    uint64_t seconds;
    unsigned nanos;
} ScoopIoDeadline;
ScoopIoDeadline scoop_io_deadline(int64_t seconds, int nanos);
int64_t scoop_io_remaining(ScoopIoDeadline deadline);
#endif
