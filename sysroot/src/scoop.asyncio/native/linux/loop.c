#include "backend.h"
#include <errno.h>
#include <poll.h>
#include <unistd.h>

ScoopIoCreated scoop_io_loop_create(void) {
    ScoopIoLoop *loop = calloc(1, sizeof(*loop));
    if (!loop)
        return (ScoopIoCreated){NULL, -ENOMEM};
    int status = pthread_mutex_init(&loop->mutex, NULL);
    if (status) {
        free(loop);
        return (ScoopIoCreated){NULL, -status};
    }
    status = pthread_cond_init(&loop->work_ready, NULL);
    if (status) {
        pthread_mutex_destroy(&loop->mutex);
        free(loop);
        return (ScoopIoCreated){NULL, -status};
    }
    status = scoop_io_ring_create(loop);
    if (status < 0) {
        pthread_cond_destroy(&loop->work_ready);
        pthread_mutex_destroy(&loop->mutex);
        free(loop);
        return (ScoopIoCreated){NULL, status};
    }
    return (ScoopIoCreated){loop, 0};
}

void scoop_io_notify_locked(ScoopIoLoop *loop) {
    uint64_t hint = 1;
    ssize_t count;
    do {
        count = write(loop->notification, &hint, sizeof(hint));
    } while (count < 0 && errno == EINTR);
    if (count < 0 && errno != EAGAIN && !loop->failure)
        loop->failure = errno;
}
void scoop_io_loop_wake(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    scoop_io_notify_locked(loop);
    pthread_mutex_unlock(&loop->mutex);
}
void scoop_io_fail(ScoopIoLoop *loop, int error) {
    pthread_mutex_lock(&loop->mutex);
    if (!loop->failure) {
        loop->failure = error;
        scoop_io_notify_locked(loop);
    }
    pthread_mutex_unlock(&loop->mutex);
}
int scoop_io_loop_descriptor(ScoopIoLoop *loop) { return loop->notification; }

int scoop_io_loop_poll(ScoopIoLoop *loop, bool wait) {
    pthread_mutex_lock(&loop->mutex);
    bool queued = loop->head || loop->finished_head || (loop->failure && !loop->failure_reported);
    pthread_mutex_unlock(&loop->mutex);
    if (wait && !queued && !io_uring_cq_ready(&loop->ring) &&
        (loop->submission_blocked ||
         (!loop->pending.head && !loop->control.head && !io_uring_sq_ready(&loop->ring)))) {
        struct pollfd descriptor = {.fd = loop->notification, .events = POLLIN};
        int status;
        do {
            status = poll(&descriptor, 1, -1);
        } while (status < 0 && errno == EINTR);
        if (status < 0)
            scoop_io_fail(loop, errno);
        else if (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL))
            scoop_io_fail(loop, EIO);
    }
    uint64_t hints;
    ssize_t count;
    do {
        count = read(loop->notification, &hints, sizeof(hints));
    } while (count < 0 && errno == EINTR);
    if (count < 0 && errno != EAGAIN)
        scoop_io_fail(loop, errno);
    scoop_io_work_poll(loop);
    scoop_io_ring_poll(loop);
    pthread_mutex_lock(&loop->mutex);
    int status = loop->failure_reported ? 0 : -loop->failure;
    loop->failure_reported = loop->failure != 0;
    pthread_mutex_unlock(&loop->mutex);
    return status;
}

void scoop_io_complete(ScoopIoLoop *loop, ScoopIoCompletion *completion, int status) {
    if (status < 0) {
        completion->event.kind = SCOOP_IO_POSIX_ERROR;
        completion->event.code = -status;
    }
    completion->next = NULL;
    pthread_mutex_lock(&loop->mutex);
    if (loop->tail)
        loop->tail->next = completion;
    else
        loop->head = completion;
    loop->tail = completion;
    loop->completions++;
    scoop_io_notify_locked(loop);
    pthread_mutex_unlock(&loop->mutex);
}
int64_t scoop_io_loop_batch_size(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    int64_t count = loop->completions;
    pthread_mutex_unlock(&loop->mutex);
    return count;
}
ScoopIoEvent scoop_io_loop_next(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    ScoopIoCompletion *entry = loop->head;
    if (entry) {
        loop->head = entry->next;
        if (!loop->head)
            loop->tail = NULL;
        loop->completions--;
    }
    pthread_mutex_unlock(&loop->mutex);
    if (!entry)
        return (ScoopIoEvent){0};
    ScoopIoEvent event = entry->event;
    free(entry);
    return event;
}

bool scoop_io_loop_idle(ScoopIoLoop *loop) {
    if (loop->requests || loop->pending.head || loop->control.head ||
        io_uring_sq_ready(&loop->ring) || io_uring_cq_ready(&loop->ring))
        return false;
    pthread_mutex_lock(&loop->mutex);
    bool idle = !loop->head && !loop->work;
    if (idle && !loop->stopping) {
        loop->stopping = true;
        pthread_cond_broadcast(&loop->work_ready);
    }
    idle = idle && !loop->workers;
    pthread_mutex_unlock(&loop->mutex);
    return idle;
}
int scoop_io_loop_destroy(ScoopIoLoop *loop) {
    if (!scoop_io_loop_idle(loop))
        return -EBUSY;
    io_uring_unregister_eventfd(&loop->ring);
    io_uring_queue_exit(&loop->ring);
    int status = close(loop->notification) < 0 ? -errno : 0;
    pthread_cond_destroy(&loop->work_ready);
    pthread_mutex_destroy(&loop->mutex);
    free(loop);
    return status;
}
ScoopIoEvent scoop_io_status_error(int status) {
    return (ScoopIoEvent){.kind = SCOOP_IO_POSIX_ERROR, .code = -status};
}
