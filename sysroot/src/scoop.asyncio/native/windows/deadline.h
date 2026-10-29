#ifndef SCOOP_ASYNCIO_WINDOWS_DEADLINE_H
#define SCOOP_ASYNCIO_WINDOWS_DEADLINE_H

#include "backend.h"

typedef struct {
    uint64_t seconds, ticks;
} ScoopIoDeadline;

static inline ScoopIoDeadline scoop_io_deadline_after(uint64_t hz, uint64_t now, int64_t seconds,
                                                      int nanos) {
    uint64_t fraction = (hz / 1000000000) * (unsigned)nanos +
                        ((hz % 1000000000) * (unsigned)nanos + 999999999) / 1000000000;
    uint64_t ticks = now % hz + fraction;
    return (ScoopIoDeadline){now / hz + (uint64_t)seconds + ticks / hz, ticks % hz};
}

static inline bool scoop_io_deadline_before(ScoopIoDeadline left, ScoopIoDeadline right) {
    return left.seconds < right.seconds ||
           (left.seconds == right.seconds && left.ticks < right.ticks);
}

/* Round up, then recheck QPC after waking; a DWORD wait never completes a timer early. */
static inline DWORD scoop_io_deadline_wait(ScoopIoDeadline deadline, uint64_t hz, uint64_t now) {
    ScoopIoDeadline current = {now / hz, now % hz};
    if (!scoop_io_deadline_before(current, deadline))
        return 0;
    uint64_t remaining = deadline.seconds - current.seconds;
    int64_t fraction = (int64_t)deadline.ticks - (int64_t)current.ticks;
    if (fraction < 0) {
        remaining--;
        fraction += (int64_t)hz;
    }
    if (remaining > (INFINITE - 1) / 1000)
        return INFINITE - 1;
    long double millis = (long double)fraction * 1000 / hz;
    uint64_t rounded = (uint64_t)millis;
    if ((long double)rounded < millis)
        rounded++;
    uint64_t total = remaining * 1000 + rounded;
    return total >= INFINITE ? INFINITE - 1 : total ? (DWORD)total : 1;
}

#endif
