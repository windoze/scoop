#include "backend.h"
#include <errno.h>

struct ScoopIoTimer {
    ScoopIoCompletion completion;
    ScoopIoTimer *next;
    uint64_t seconds, ticks;
};

static bool before(uint64_t seconds, uint64_t ticks, const ScoopIoTimer *timer) {
    return seconds < timer->seconds || (seconds == timer->seconds && ticks < timer->ticks);
}

ScoopIoStatus scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos) {
    if (seconds < 0 || nanos < 0 || nanos >= 1000000000)
        return scoop_io_win32(ERROR_INVALID_PARAMETER);
    LARGE_INTEGER now;
    if (!QueryPerformanceCounter(&now))
        return scoop_io_win32(GetLastError());
    ScoopIoTimer *timer = calloc(1, sizeof(*timer));
    if (!timer)
        return scoop_io_errno(-ENOMEM);
    uint64_t hz = loop->frequency;
    uint64_t fraction = (hz / 1000000000) * (unsigned)nanos +
                        ((hz % 1000000000) * (unsigned)nanos + 999999999) / 1000000000;
    uint64_t ticks = (uint64_t)now.QuadPart % hz + fraction;
    timer->seconds = (uint64_t)now.QuadPart / hz + (uint64_t)seconds + ticks / hz;
    timer->ticks = ticks % hz;
    timer->completion.event.id = id;
    AcquireSRWLockExclusive(&loop->mutex);
    ScoopIoTimer **entry = &loop->timers;
    while (*entry && !before(timer->seconds, timer->ticks, *entry))
        entry = &(*entry)->next;
    timer->next = *entry;
    *entry = timer;
    scoop_io_collector_wake_locked(loop);
    ReleaseSRWLockExclusive(&loop->mutex);
    return (ScoopIoStatus){0};
}

void scoop_io_timer_cancel(ScoopIoLoop *loop, int64_t id) {
    AcquireSRWLockExclusive(&loop->mutex);
    ScoopIoTimer **entry = &loop->timers;
    while (*entry && (*entry)->completion.event.id != id)
        entry = &(*entry)->next;
    if (*entry) {
        ScoopIoTimer *timer = *entry;
        *entry = timer->next;
        scoop_io_complete_locked(loop, &timer->completion);
        scoop_io_collector_wake_locked(loop);
    }
    ReleaseSRWLockExclusive(&loop->mutex);
}

DWORD scoop_io_timers_poll_locked(ScoopIoLoop *loop) {
    if (!loop->timers)
        return INFINITE;
    LARGE_INTEGER now;
    if (!QueryPerformanceCounter(&now)) {
        DWORD error = GetLastError();
        while (loop->timers) {
            ScoopIoTimer *timer = loop->timers;
            loop->timers = timer->next;
            timer->completion.event.kind = SCOOP_IO_WIN32_ERROR;
            timer->completion.event.code = (int32_t)error;
            scoop_io_complete_locked(loop, &timer->completion);
        }
        return INFINITE;
    }
    uint64_t seconds = (uint64_t)now.QuadPart / loop->frequency;
    uint64_t ticks = (uint64_t)now.QuadPart % loop->frequency;
    while (loop->timers && !before(seconds, ticks, loop->timers)) {
        ScoopIoTimer *timer = loop->timers;
        loop->timers = timer->next;
        scoop_io_complete_locked(loop, &timer->completion);
    }
    if (!loop->timers)
        return INFINITE;
    uint64_t remaining = loop->timers->seconds - seconds;
    int64_t fraction = (int64_t)loop->timers->ticks - (int64_t)ticks;
    if (fraction < 0) {
        remaining--;
        fraction += (int64_t)loop->frequency;
    }
    if (remaining > (INFINITE - 1) / 1000)
        return INFINITE - 1;
    long double millis = (long double)fraction * 1000 / loop->frequency;
    uint64_t rounded = (uint64_t)millis;
    if ((long double)rounded < millis)
        rounded++;
    uint64_t total = remaining * 1000 + rounded;
    return total >= INFINITE ? INFINITE - 1 : total ? (DWORD)total : 1;
}
