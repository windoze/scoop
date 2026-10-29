#include "deadline.h"
#include <errno.h>

struct ScoopIoTimer {
    ScoopIoCompletion completion;
    ScoopIoTimer *next;
    ScoopIoDeadline deadline;
};

ScoopIoStatus scoop_io_timer_start(ScoopIoLoop *loop, int64_t id, int64_t seconds, int nanos) {
    if (seconds < 0 || nanos < 0 || nanos >= 1000000000)
        return scoop_io_win32(ERROR_INVALID_PARAMETER);
    LARGE_INTEGER now;
    if (!QueryPerformanceCounter(&now))
        return scoop_io_win32(GetLastError());
    ScoopIoTimer *timer = calloc(1, sizeof(*timer));
    if (!timer)
        return scoop_io_errno(-ENOMEM);
    timer->deadline =
        scoop_io_deadline_after(loop->frequency, (uint64_t)now.QuadPart, seconds, nanos);
    timer->completion.event.id = id;
    AcquireSRWLockExclusive(&loop->mutex);
    ScoopIoTimer **entry = &loop->timers;
    while (*entry && !scoop_io_deadline_before(timer->deadline, (*entry)->deadline))
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
    while (loop->timers && !scoop_io_deadline_wait(loop->timers->deadline, loop->frequency,
                                                   (uint64_t)now.QuadPart)) {
        ScoopIoTimer *timer = loop->timers;
        loop->timers = timer->next;
        scoop_io_complete_locked(loop, &timer->completion);
    }
    if (!loop->timers)
        return INFINITE;
    return scoop_io_deadline_wait(loop->timers->deadline, loop->frequency, (uint64_t)now.QuadPart);
}
