#ifndef SCOOP_ASYNCIO_WINDOWS_BACKEND_H
#define SCOOP_ASYNCIO_WINDOWS_BACKEND_H

#include "../../../scoop.platform/native/windows_api.h"
#include "../common/work.h"

typedef struct ScoopIoRequest ScoopIoRequest;
typedef struct ScoopIoTimer ScoopIoTimer;

/* Owner owns the request list; the collector only publishes completed packets under the lock. */
struct ScoopIoRequest {
    ScoopIoCompletion completion;
    OVERLAPPED overlapped;
    ScoopIoLoop *loop;
    ScoopIoRequest *next, *packet_next;
    HANDLE handle;
    void (*finish)(ScoopIoRequest *, DWORD, DWORD);
    DWORD count, error;
    bool cancelled;
};

struct ScoopIoLoop {
    SRWLOCK mutex;
    CONDITION_VARIABLE work_ready;
    HANDLE port, notification;
    uint64_t frequency;
    ScoopIoStatus failure;
    bool failure_reported, stopping, collecting, workers_stopping, workers_started;
    unsigned workers;
    int64_t completions;
    ScoopIoCompletion *head, *tail;
    ScoopIoRequest *requests, *packets_head, *packets_tail;
    ScoopIoTimer *timers;
    ScoopIoWork *work, *pending_head, *pending_tail, *finished_head, *finished_tail;
};

static inline ScoopIoStatus scoop_io_win32(DWORD code) {
    return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, (int32_t)code};
}

void scoop_io_notify_locked(ScoopIoLoop *loop);
void scoop_io_fail_locked(ScoopIoLoop *loop, ScoopIoStatus error);
void scoop_io_complete_locked(ScoopIoLoop *loop, ScoopIoCompletion *completion);
void scoop_io_collector_wake_locked(ScoopIoLoop *loop);
unsigned __stdcall scoop_io_collect(void *context);
DWORD scoop_io_timers_poll_locked(ScoopIoLoop *loop);
void scoop_io_request_init(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id, HANDLE handle,
                           void (*finish)(ScoopIoRequest *, DWORD, DWORD));
void scoop_io_request_retire(ScoopIoRequest *request);
void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id);
void scoop_io_work_poll(ScoopIoWork *work);

#endif
