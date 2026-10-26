#ifndef SCOOP_ASYNCIO_LINUX_BACKEND_H
#define SCOOP_ASYNCIO_LINUX_BACKEND_H

#include "../common/work.h"
#include <liburing.h>
#include <pthread.h>

typedef struct ScoopIoRequest ScoopIoRequest;
typedef enum { RING_UNUSED, RING_QUEUED, RING_PUBLISHED, RING_FINISHED } RingState;
typedef struct RingEntry {
    struct RingEntry *next;
    ScoopIoRequest *request;
    unsigned index;
    RingState state;
} RingEntry;
typedef struct {
    RingEntry *head, *tail;
} RingPending;

struct ScoopIoRequest {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    ScoopIoRequest *next;
    void (*prepare)(ScoopIoRequest *, unsigned, struct io_uring_sqe *);
    void (*result)(ScoopIoRequest *, unsigned, int);
    void (*finish)(ScoopIoRequest *);
    RingEntry entries[6]; // Three target slots and their independently retired cancel requests.
    unsigned live;
    bool ended, retired, cancelled;
};

struct ScoopIoLoop {
    struct io_uring ring;
    int notification, failure;
    pthread_mutex_t mutex;
    pthread_cond_t work_ready;
    unsigned workers;
    bool stopping, workers_started, failure_reported, submission_blocked;
    int64_t completions;
    ScoopIoCompletion *head, *tail;
    ScoopIoRequest *requests;
    RingPending control, pending;
    ScoopIoWork *work, *pending_head, *pending_tail, *finished_head, *finished_tail;
};

void scoop_io_notify_locked(ScoopIoLoop *loop);
void scoop_io_fail(ScoopIoLoop *loop, int error);
int scoop_io_ring_capabilities(const struct io_uring_probe *probe, unsigned features);
int scoop_io_ring_create(ScoopIoLoop *loop);
void scoop_io_ring_poll(ScoopIoLoop *loop);
void scoop_io_request_init(ScoopIoRequest *request, ScoopIoLoop *loop, int64_t id);
void scoop_io_request_queue(ScoopIoRequest *request, unsigned index);
void scoop_io_request_end(ScoopIoRequest *request);
void scoop_io_request_cancel(ScoopIoLoop *loop, int64_t id);
void scoop_io_request_error(ScoopIoRequest *request, int error);
void scoop_io_entry_complete(RingEntry *entry, int result);
int scoop_io_workers_start(ScoopIoLoop *loop);
void scoop_io_work_poll(ScoopIoLoop *loop);

#endif
