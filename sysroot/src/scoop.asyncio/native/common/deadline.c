#include "deadline.h"
#include <limits.h>

ScoopIoDeadline scoop_io_deadline(int64_t seconds, int nanos) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    uint64_t sum = (uint64_t)now.tv_sec + (uint64_t)seconds;
    unsigned part = (unsigned)now.tv_nsec + (unsigned)nanos;
    return (ScoopIoDeadline){sum + part / 1000000000u, part % 1000000000u};
}

int64_t scoop_io_remaining(ScoopIoDeadline deadline) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    if (deadline.seconds < (uint64_t)now.tv_sec ||
        (deadline.seconds == (uint64_t)now.tv_sec && deadline.nanos <= (unsigned)now.tv_nsec))
        return 0;
    uint64_t seconds = deadline.seconds - (uint64_t)now.tv_sec;
    int64_t nanos = (int64_t)deadline.nanos - now.tv_nsec;
    if (nanos < 0) {
        seconds--;
        nanos += 1000000000;
    }
    if (seconds > ((uint64_t)INT64_MAX - (uint64_t)nanos) / 1000000000)
        return INT64_MAX;
    return (int64_t)seconds * 1000000000 + nanos;
}
