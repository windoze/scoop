#include "backend.h"
#include <errno.h>
#include <process.h>

ScoopIoCreated scoop_io_loop_create(void) {
    ScoopIoLoop *loop = calloc(1, sizeof(*loop));
    if (!loop)
        return (ScoopIoCreated){NULL, scoop_io_errno(-ENOMEM)};
    InitializeSRWLock(&loop->mutex);
    InitializeConditionVariable(&loop->work_ready);
    LARGE_INTEGER frequency;
    ScoopIoStatus failure = {0};
    if (!QueryPerformanceFrequency(&frequency)) {
        failure = scoop_io_win32(GetLastError());
        goto failed;
    }
    loop->frequency = (uint64_t)frequency.QuadPart;
    loop->port = CreateIoCompletionPort(INVALID_HANDLE_VALUE, NULL, 0, 1);
    if (!loop->port) {
        failure = scoop_io_win32(GetLastError());
        goto failed;
    }
    loop->notification = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (!loop->notification) {
        failure = scoop_io_win32(GetLastError());
        goto failed;
    }
    loop->collecting = true;
    uintptr_t thread = _beginthreadex(NULL, 0, scoop_io_collect, loop, 0, NULL);
    if (!thread) {
        failure = scoop_io_errno(-errno);
        goto failed;
    }
    CloseHandle((HANDLE)thread);
    return (ScoopIoCreated){loop, {0}};
failed:
    if (loop->notification)
        CloseHandle(loop->notification);
    if (loop->port)
        CloseHandle(loop->port);
    free(loop);
    return (ScoopIoCreated){NULL, failure};
}

void scoop_io_fail_locked(ScoopIoLoop *loop, ScoopIoStatus error) {
    if (!loop->failure.code)
        loop->failure = error;
}

void scoop_io_notify_locked(ScoopIoLoop *loop) {
    if (!SetEvent(loop->notification))
        scoop_io_fail_locked(loop, scoop_io_win32(GetLastError()));
}

void scoop_io_loop_wake(ScoopIoLoop *loop) {
    AcquireSRWLockExclusive(&loop->mutex);
    scoop_io_notify_locked(loop);
    ReleaseSRWLockExclusive(&loop->mutex);
}

void *scoop_io_loop_event(ScoopIoLoop *loop) { return loop->notification; }

ScoopIoStatus scoop_io_loop_poll(ScoopIoLoop *loop, bool wait) {
    AcquireSRWLockExclusive(&loop->mutex);
    bool queued = loop->head || loop->packets_head || loop->finished_head ||
                  (loop->failure.code && !loop->failure_reported);
    ReleaseSRWLockExclusive(&loop->mutex);
    if (wait && !queued && WaitForSingleObject(loop->notification, INFINITE) == WAIT_FAILED) {
        DWORD error = GetLastError();
        AcquireSRWLockExclusive(&loop->mutex);
        scoop_io_fail_locked(loop, scoop_io_win32(error));
        ReleaseSRWLockExclusive(&loop->mutex);
    }
    AcquireSRWLockExclusive(&loop->mutex);
    if (!ResetEvent(loop->notification))
        scoop_io_fail_locked(loop, scoop_io_win32(GetLastError()));
    ScoopIoRequest *packets = loop->packets_head;
    loop->packets_head = loop->packets_tail = NULL;
    ScoopIoWork *work = loop->finished_head;
    loop->finished_head = loop->finished_tail = NULL;
    ScoopIoStatus status = loop->failure_reported ? (ScoopIoStatus){0} : loop->failure;
    loop->failure_reported = loop->failure.code != 0;
    ReleaseSRWLockExclusive(&loop->mutex);
    while (packets) {
        ScoopIoRequest *next = packets->packet_next;
        packets->finish(packets, packets->count, packets->error);
        packets = next;
    }
    scoop_io_work_poll(work);
    return status;
}

void scoop_io_complete_locked(ScoopIoLoop *loop, ScoopIoCompletion *completion) {
    completion->next = NULL;
    if (loop->tail)
        loop->tail->next = completion;
    else
        loop->head = completion;
    loop->tail = completion;
    loop->completions++;
    scoop_io_notify_locked(loop);
}

void scoop_io_complete(ScoopIoLoop *loop, ScoopIoCompletion *completion, int status) {
    if (status < 0) {
        completion->event.kind = SCOOP_IO_POSIX_ERROR;
        completion->event.code = -status;
    }
    AcquireSRWLockExclusive(&loop->mutex);
    scoop_io_complete_locked(loop, completion);
    ReleaseSRWLockExclusive(&loop->mutex);
}

int64_t scoop_io_loop_batch_size(ScoopIoLoop *loop) {
    AcquireSRWLockExclusive(&loop->mutex);
    int64_t count = loop->completions;
    ReleaseSRWLockExclusive(&loop->mutex);
    return count;
}

ScoopIoEvent scoop_io_loop_next(ScoopIoLoop *loop) {
    AcquireSRWLockExclusive(&loop->mutex);
    ScoopIoCompletion *entry = loop->head;
    if (entry) {
        loop->head = entry->next;
        if (!loop->head)
            loop->tail = NULL;
        loop->completions--;
    }
    ReleaseSRWLockExclusive(&loop->mutex);
    if (!entry)
        return (ScoopIoEvent){0};
    ScoopIoEvent event = entry->event;
    free(entry);
    return event;
}

bool scoop_io_loop_idle(ScoopIoLoop *loop) {
    if (loop->requests)
        return false;
    AcquireSRWLockExclusive(&loop->mutex);
    bool idle = !loop->head && !loop->packets_head && !loop->timers && !loop->work;
    if (idle && !loop->stopping) {
        loop->stopping = true;
        loop->workers_stopping = true;
        WakeAllConditionVariable(&loop->work_ready);
        scoop_io_collector_wake_locked(loop);
    }
    idle = idle && !loop->collecting && !loop->workers;
    ReleaseSRWLockExclusive(&loop->mutex);
    return idle;
}

ScoopIoStatus scoop_io_loop_destroy(ScoopIoLoop *loop) {
    if (!scoop_io_loop_idle(loop))
        return scoop_io_win32(ERROR_BUSY);
    DWORD error = CloseHandle(loop->port) ? 0 : GetLastError();
    if (!CloseHandle(loop->notification) && !error)
        error = GetLastError();
    free(loop);
    return scoop_io_win32(error);
}
