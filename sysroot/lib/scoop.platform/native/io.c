#define _GNU_SOURCE 1
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <pthread.h>
#include <signal.h>
#include <stdint.h>
#include <unistd.h>

static size_t transfer_size(int64_t count) {
    /* Darwin rejects counts above INT_MAX; Linux may complete a shorter IO. */
    uint64_t maximum = INT_MAX;
    return (size_t)((uint64_t)count < maximum ? (uint64_t)count : maximum);
}

int64_t scoop_platform_fd_read(int fd, void *data, int64_t count) {
    if (count < 0) {
        errno = EINVAL;
        return -1;
    }
    if (count == 0)
        return 0;
    ssize_t result;
    do {
        result = read(fd, data, transfer_size(count));
    } while (result < 0 && errno == EINTR);
    return result;
}

int64_t scoop_platform_fd_write(int fd, const void *data, int64_t count) {
    if (count < 0) {
        errno = EINVAL;
        return -1;
    }
    if (count == 0)
        return 0;
#ifdef __APPLE__
    /* XNU sends fd SIGPIPE to the process, not just the calling thread. */
    if (fcntl(fd, F_SETNOSIGPIPE, 1) < 0)
        return -1;
#else
    sigset_t blocked, previous, pending;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGPIPE);
    int error = pthread_sigmask(SIG_BLOCK, &blocked, &previous);
    if (error != 0) {
        errno = error;
        return -1;
    }
    if (sigpending(&pending) < 0) {
        int saved = errno;
        (void)pthread_sigmask(SIG_SETMASK, &previous, NULL);
        errno = saved;
        return -1;
    }
    int was_pending = sigismember(&pending, SIGPIPE);
#endif
    ssize_t result;
    do {
        result = write(fd, data, transfer_size(count));
    } while (result < 0 && errno == EINTR);
    int saved = errno;
#ifndef __APPLE__
    if (result < 0 && saved == EPIPE && !was_pending) {
        struct timespec zero = {0, 0};
        while (sigtimedwait(&blocked, NULL, &zero) < 0 && errno == EINTR) {
        }
    }
    /* Valid masks on this thread cannot fail restoration. Keep the IO errno. */
    (void)pthread_sigmask(SIG_SETMASK, &previous, NULL);
#endif
    errno = saved;
    return result;
}

int scoop_platform_fd_close(int fd) { return close(fd); }
int scoop_platform_fd_duplicate(int fd) { return fcntl(fd, F_DUPFD_CLOEXEC, 0); }
