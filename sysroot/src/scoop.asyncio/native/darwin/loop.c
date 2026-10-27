#include "backend.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/ioctl.h>
#include <unistd.h>

static int notification_pipe(int descriptors[2]) {
    if (pipe(descriptors) < 0)
        return -errno;
    for (unsigned i = 0; i < 2; i++) {
        if (fcntl(descriptors[i], F_SETFD, FD_CLOEXEC) < 0 ||
            fcntl(descriptors[i], F_SETFL, O_NONBLOCK) < 0) {
            int error = errno;
            close(descriptors[0]);
            close(descriptors[1]);
            return -error;
        }
    }
    return 0;
}

ScoopIoCreated scoop_io_loop_create(void) {
    ScoopIoLoop *loop = calloc(1, sizeof(*loop));
    if (!loop)
        return (ScoopIoCreated){NULL, -ENOMEM};
    int status = pthread_mutex_init(&loop->mutex, NULL);
    if (status) {
        free(loop);
        return (ScoopIoCreated){NULL, -status};
    }
    int descriptors[2];
    status = notification_pipe(descriptors);
    if (status < 0) {
        pthread_mutex_destroy(&loop->mutex);
        free(loop);
        return (ScoopIoCreated){NULL, status};
    }
    loop->wake_read = descriptors[0];
    loop->wake_write = descriptors[1];
    loop->queue = dispatch_queue_create("scoop.asyncio.io", DISPATCH_QUEUE_SERIAL);
    if (!loop->queue) {
        close(loop->wake_read);
        close(loop->wake_write);
        pthread_mutex_destroy(&loop->mutex);
        free(loop);
        return (ScoopIoCreated){NULL, -ENOMEM};
    }
    return (ScoopIoCreated){loop, 0};
}

void scoop_io_notify_locked(ScoopIoLoop *loop) {
    char hint = 1;
    ssize_t count;
    do {
        count = write(loop->wake_write, &hint, 1);
    } while (count < 0 && errno == EINTR);
    if (count < 0 && errno != EAGAIN && errno != EWOULDBLOCK && !loop->failure)
        loop->failure = errno;
}

void scoop_io_loop_wake(ScoopIoLoop *loop) {
    pthread_mutex_lock(&loop->mutex);
    scoop_io_notify_locked(loop);
    pthread_mutex_unlock(&loop->mutex);
}

int scoop_io_loop_descriptor(ScoopIoLoop *loop) { return loop->wake_read; }

int scoop_io_loop_poll(ScoopIoLoop *loop, bool wait) {
    if (wait) {
        pthread_mutex_lock(&loop->mutex);
        bool pending = loop->head || loop->finished_head || loop->failure;
        pthread_mutex_unlock(&loop->mutex);
        if (!pending) {
            struct pollfd descriptor = {.fd = loop->wake_read, .events = POLLIN};
            int status;
            do {
                status = poll(&descriptor, 1, -1);
            } while (status < 0 && errno == EINTR);
            if (status < 0)
                return -errno;
            if (descriptor.revents & (POLLERR | POLLHUP | POLLNVAL))
                return -EIO;
        }
    }
    int available = 0;
    if (ioctl(loop->wake_read, FIONREAD, &available) < 0)
        return -errno;
    char hints[256];
    while (available > 0) {
        size_t wanted = (size_t)available < sizeof(hints) ? (size_t)available : sizeof(hints);
        ssize_t count = read(loop->wake_read, hints, wanted);
        if (count > 0) {
            available -= (int)count;
            continue;
        }
        if (count < 0 && errno == EINTR)
            continue;
        if (count < 0 && (errno == EAGAIN || errno == EWOULDBLOCK))
            break;
        return count < 0 ? -errno : -EIO;
    }
    scoop_io_work_poll(loop);
    pthread_mutex_lock(&loop->mutex);
    int status = -loop->failure;
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
    pthread_mutex_lock(&loop->mutex);
    bool idle = !loop->head && !loop->requests && !loop->work && !loop->actions && !loop->workers;
    pthread_mutex_unlock(&loop->mutex);
    return idle;
}

int scoop_io_loop_destroy(ScoopIoLoop *loop) {
    if (!scoop_io_loop_idle(loop))
        return -EBUSY;
    dispatch_release(loop->queue);
    int error = close(loop->wake_read) < 0 ? errno : 0;
    if (close(loop->wake_write) < 0 && !error)
        error = errno;
    pthread_mutex_destroy(&loop->mutex);
    free(loop);
    return -error;
}
