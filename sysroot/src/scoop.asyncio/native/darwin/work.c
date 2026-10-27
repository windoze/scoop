#include "backend.h"
#include <errno.h>

// A small set of dispatch jobs drains the FIFO. No job blocks the serial I/O queue.
unsigned scoop_io_work_width(void) { return 4; }

static void append_finished(ScoopIoLoop *loop, ScoopIoWork *work) {
    work->state = SCOOP_WORK_FINISHED;
    work->queued = NULL;
    if (loop->finished_tail)
        loop->finished_tail->queued = work;
    else
        loop->finished_head = work;
    loop->finished_tail = work;
    scoop_io_notify_locked(loop);
}

static void execute_work(void *context) {
    ScoopIoLoop *loop = context;
    for (;;) {
        pthread_mutex_lock(&loop->mutex);
        ScoopIoWork *work = loop->pending_head;
        if (!work) {
            loop->workers--;
            scoop_io_notify_locked(loop);
            pthread_mutex_unlock(&loop->mutex);
            return;
        }
        loop->pending_head = work->queued;
        if (!loop->pending_head)
            loop->pending_tail = NULL;
        work->state = SCOOP_WORK_RUNNING;
        pthread_mutex_unlock(&loop->mutex);
        work->execute(work);
        pthread_mutex_lock(&loop->mutex);
        append_finished(loop, work);
        pthread_mutex_unlock(&loop->mutex);
    }
}

int scoop_io_work_submit(ScoopIoWork *work, ScoopIoLoop *loop, int64_t id,
                         ScoopIoWorkExecute execute, ScoopIoWorkFinish finish) {
    work->loop = loop;
    work->completion.event.id = id;
    work->execute = execute;
    work->finish = finish;
    work->state = SCOOP_WORK_QUEUED;
    pthread_mutex_lock(&loop->mutex);
    work->next = loop->work;
    loop->work = work;
    work->queued = NULL;
    if (loop->pending_tail)
        loop->pending_tail->queued = work;
    else
        loop->pending_head = work;
    loop->pending_tail = work;
    if (loop->workers < scoop_io_work_width()) {
        loop->workers++;
        dispatch_async_f(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), loop, execute_work);
    }
    pthread_mutex_unlock(&loop->mutex);
    return 0;
}

void scoop_io_work_cancel(ScoopIoLoop *loop, int64_t id) {
    pthread_mutex_lock(&loop->mutex);
    ScoopIoWork *previous = NULL;
    ScoopIoWork *work = loop->pending_head;
    while (work && work->completion.event.id != id) {
        previous = work;
        work = work->queued;
    }
    if (work) {
        if (previous)
            previous->queued = work->queued;
        else
            loop->pending_head = work->queued;
        if (loop->pending_tail == work)
            loop->pending_tail = previous;
        work->status = -ECANCELED;
        append_finished(loop, work);
    }
    pthread_mutex_unlock(&loop->mutex);
}

void scoop_io_work_poll(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    ScoopIoWork *work = loop->finished_head;
    loop->finished_head = loop->finished_tail = NULL;
    pthread_mutex_unlock(&loop->mutex);
    while (work) {
        ScoopIoWork *next = work->queued;
        work->finish(work, work->status);
        work = next;
    }
}

void scoop_io_work_retire(ScoopIoWork *work) {
    ScoopIoLoop *loop = work->loop;
    pthread_mutex_lock(&loop->mutex);
    ScoopIoWork **entry = &loop->work;
    while (*entry != work)
        entry = &(*entry)->next;
    *entry = work->next;
    pthread_mutex_unlock(&loop->mutex);
}

void scoop_io_work_finished(ScoopIoWork *work, int status) {
    scoop_io_work_retire(work);
    scoop_io_complete(work->loop, &work->completion, status);
}
