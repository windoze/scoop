#ifndef SCOOP_ASYNCIO_DARWIN_BACKEND_H
#define SCOOP_ASYNCIO_DARWIN_BACKEND_H

#include "../common/work.h"
#include <dispatch/dispatch.h>
#include <pthread.h>

// Only the serial I/O queue mutates request payloads and dispatch sources.
// The owner publishes requests and cancellation flags under the loop mutex.
typedef struct ScoopIoRequest {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    struct ScoopIoRequest *next;
    void (*cancel)(struct ScoopIoRequest *);
    bool started, cancelled, cancel_sent;
} ScoopIoRequest;

struct ScoopIoLoop {
    pthread_mutex_t mutex;
    dispatch_queue_t queue;
    int wake_read, wake_write, failure;
    unsigned actions, workers;
    int64_t completions;
    ScoopIoCompletion *head, *tail;
    ScoopIoRequest *requests;
    ScoopIoWork *work, *pending_head, *pending_tail, *finished_head, *finished_tail;
};

void scoop_io_action_begin(ScoopIoLoop *loop);
void scoop_io_action_end(ScoopIoLoop *loop);
void scoop_io_request_submit(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id,
                             dispatch_function_t start, void (*cancel)(ScoopIoRequest *));
bool scoop_io_request_start(ScoopIoRequest *request);
void scoop_io_request_finish(ScoopIoRequest *request);
void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id);
void scoop_io_work_poll(ScoopIoLoop *loop);
void scoop_io_notify_locked(ScoopIoLoop *loop);

#endif
