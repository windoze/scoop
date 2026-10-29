#include "backend.h"
#include <stddef.h>

void scoop_io_collector_wake_locked(ScoopIoLoop *loop) {
    if (!PostQueuedCompletionStatus(loop->port, 0, 0, NULL)) {
        scoop_io_fail_locked(loop, scoop_io_win32(GetLastError()));
        scoop_io_notify_locked(loop);
    }
}

unsigned __stdcall scoop_io_collect(void *context) {
    ScoopIoLoop *loop = context;
    AcquireSRWLockExclusive(&loop->mutex);
    while (!loop->stopping) {
        DWORD timeout = scoop_io_timers_poll_locked(loop);
        ReleaseSRWLockExclusive(&loop->mutex);
        DWORD count = 0;
        ULONG_PTR key = 0;
        OVERLAPPED *overlapped = NULL;
        BOOL success = GetQueuedCompletionStatus(loop->port, &count, &key, &overlapped, timeout);
        DWORD error = success ? 0 : GetLastError();
        AcquireSRWLockExclusive(&loop->mutex);
        if (overlapped) {
            ScoopIoRequest *request = CONTAINING_RECORD(overlapped, ScoopIoRequest, overlapped);
            request->count = count;
            request->error = error;
            request->packet_next = NULL;
            if (loop->packets_tail)
                loop->packets_tail->packet_next = request;
            else
                loop->packets_head = request;
            loop->packets_tail = request;
            scoop_io_notify_locked(loop);
        } else if (error && error != WAIT_TIMEOUT) {
            scoop_io_fail_locked(loop, scoop_io_win32(error));
            break;
        }
    }
    /* No access to loop occurs after releasing this lock, including during CRT thread teardown. */
    loop->collecting = false;
    scoop_io_notify_locked(loop);
    ReleaseSRWLockExclusive(&loop->mutex);
    return 0;
}

void scoop_io_request_init(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id, HANDLE handle,
                           void (*finish)(ScoopIoRequest *, DWORD, DWORD)) {
    request->loop = loop;
    request->handle = handle;
    request->finish = finish;
    request->completion.event.id = id;
    request->next = loop->requests;
    loop->requests = request;
}

void scoop_io_request_retire(ScoopIoRequest *request) {
    ScoopIoRequest **entry = &request->loop->requests;
    while (*entry != request)
        entry = &(*entry)->next;
    *entry = request->next;
}

void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id) {
    for (ScoopIoRequest *request = loop->requests; request; request = request->next) {
        if (request->completion.event.id != id || request->cancelled)
            continue;
        request->cancelled = true;
        if (!CancelIoEx(request->handle, &request->overlapped)) {
            DWORD error = GetLastError();
            if (error != ERROR_NOT_FOUND) {
                AcquireSRWLockExclusive(&loop->mutex);
                scoop_io_fail_locked(loop, scoop_io_win32(error));
                scoop_io_notify_locked(loop);
                ReleaseSRWLockExclusive(&loop->mutex);
            }
        }
        return;
    }
}
