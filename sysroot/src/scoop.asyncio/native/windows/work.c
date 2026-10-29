#include "backend.h"
#include <errno.h>
#include <process.h>

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

static unsigned __stdcall execute_work(void *context) {
    ScoopIoLoop *loop = context;
    for (;;) {
        AcquireSRWLockExclusive(&loop->mutex);
        while (!loop->pending_head && !loop->workers_stopping)
            SleepConditionVariableSRW(&loop->work_ready, &loop->mutex, INFINITE, 0);
        ScoopIoWork *work = loop->pending_head;
        if (!work) {
            loop->workers--;
            scoop_io_notify_locked(loop);
            ReleaseSRWLockExclusive(&loop->mutex);
            return 0;
        }
        loop->pending_head = work->queued;
        if (!loop->pending_head)
            loop->pending_tail = NULL;
        work->state = SCOOP_WORK_RUNNING;
        ReleaseSRWLockExclusive(&loop->mutex);
        work->execute(work);
        AcquireSRWLockExclusive(&loop->mutex);
        append_finished(loop, work);
        ReleaseSRWLockExclusive(&loop->mutex);
    }
}

static int workers_start(ScoopIoLoop *loop) {
    AcquireSRWLockExclusive(&loop->mutex);
    if (loop->workers) {
        ReleaseSRWLockExclusive(&loop->mutex);
        return -EAGAIN;
    }
    loop->workers_stopping = false;
    for (unsigned i = 0; i < scoop_io_work_width(); i++) {
        uintptr_t thread = _beginthreadex(NULL, 0, execute_work, loop, 0, NULL);
        if (!thread) {
            int error = errno;
            loop->workers_stopping = true;
            WakeAllConditionVariable(&loop->work_ready);
            ReleaseSRWLockExclusive(&loop->mutex);
            return -error;
        }
        loop->workers++;
        CloseHandle((HANDLE)thread);
    }
    loop->workers_started = true;
    ReleaseSRWLockExclusive(&loop->mutex);
    return 0;
}

int scoop_io_work_submit(ScoopIoWork *work, ScoopIoLoop *loop, int64_t id,
                         ScoopIoWorkExecute execute, ScoopIoWorkFinish finish) {
    if (!loop->workers_started) {
        int status = workers_start(loop);
        if (status < 0)
            return status;
    }
    work->loop = loop;
    work->completion.event.id = id;
    work->execute = execute;
    work->finish = finish;
    work->state = SCOOP_WORK_QUEUED;
    AcquireSRWLockExclusive(&loop->mutex);
    work->next = loop->work;
    loop->work = work;
    work->queued = NULL;
    if (loop->pending_tail)
        loop->pending_tail->queued = work;
    else
        loop->pending_head = work;
    loop->pending_tail = work;
    WakeConditionVariable(&loop->work_ready);
    ReleaseSRWLockExclusive(&loop->mutex);
    return 0;
}

void scoop_io_work_cancel(ScoopIoLoop *loop, int64_t id) {
    scoop_io_request_cancel(loop, id);
    AcquireSRWLockExclusive(&loop->mutex);
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
    ReleaseSRWLockExclusive(&loop->mutex);
}

void scoop_io_work_poll(ScoopIoWork *work) {
    while (work) {
        ScoopIoWork *next = work->queued;
        work->finish(work, work->status);
        work = next;
    }
}

void scoop_io_work_retire(ScoopIoWork *work) {
    ScoopIoLoop *loop = work->loop;
    AcquireSRWLockExclusive(&loop->mutex);
    ScoopIoWork **entry = &loop->work;
    while (*entry != work)
        entry = &(*entry)->next;
    *entry = work->next;
    ReleaseSRWLockExclusive(&loop->mutex);
}

void scoop_io_work_finished(ScoopIoWork *work, int status) {
    scoop_io_work_retire(work);
    scoop_io_complete(work->loop, &work->completion, status);
}
