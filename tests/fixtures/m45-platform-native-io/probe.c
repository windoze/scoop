#define _GNU_SOURCE 1
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static int interrupt_read, interrupt_write;
static ssize_t injected_read(int fd, void *data, size_t count) {
    if (interrupt_read) {
        interrupt_read = 0;
        errno = EINTR;
        return -1;
    }
    return read(fd, data, count);
}
static ssize_t injected_write(int fd, const void *data, size_t count) {
    if (interrupt_write) {
        interrupt_write = 0;
        errno = EINTR;
        return -1;
    }
    return write(fd, data, count);
}
#define read injected_read
#define write injected_write
#include "io.c"
#undef read
#undef write

static int temporary_creates;
static int checked_mkostemp(char *pattern, int flags) {
    int fd = mkostemp(pattern, flags);
    assert(fd >= 0 && (fcntl(fd, F_GETFD) & FD_CLOEXEC));
    ++temporary_creates;
    return fd;
}
#define mkostemp checked_mkostemp
#include "paths.c"
#undef mkostemp

static volatile sig_atomic_t signals;
static void handler(int signal) {
    (void)signal;
    signals++;
}

static void byte_transfers(void) {
    int descriptors[2];
    assert(pipe(descriptors) == 0);
    interrupt_write = 1;
    assert(scoop_platform_fd_write(descriptors[1], "abc", 3) == 3);
    char bytes[8] = {0};
    interrupt_read = 1;
    assert(scoop_platform_fd_read(descriptors[0], bytes, sizeof(bytes)) == 3);
    assert(memcmp(bytes, "abc", 3) == 0);
    assert(!interrupt_read && !interrupt_write);
    int copy = scoop_platform_fd_duplicate(descriptors[0]);
    assert(copy >= 0 && (fcntl(copy, F_GETFD) & FD_CLOEXEC));
    assert(scoop_platform_fd_close(copy) == 0);
    assert(fcntl(descriptors[1], F_SETFL, O_NONBLOCK) == 0);
    size_t count = 1024 * 1024;
    char *large = calloc(count, 1);
    assert(large != NULL);
    int64_t written = scoop_platform_fd_write(descriptors[1], large, (int64_t)count);
    assert(written > 0 && written < (int64_t)count);
    free(large);
    assert(close(descriptors[0]) == 0);
    assert(close(descriptors[1]) == 0);
    assert(scoop_platform_fd_read(-1, NULL, 0) == 0);
    assert(scoop_platform_fd_write(-1, NULL, 0) == 0);
    assert(scoop_platform_fd_read(-1, NULL, -1) == -1 && errno == EINVAL);
}

static void broken_pipe(void) {
    struct sigaction action = {0}, old_action, current;
    action.sa_handler = handler;
    sigemptyset(&action.sa_mask);
    assert(sigaction(SIGPIPE, &action, &old_action) == 0);
    int descriptors[2];
    assert(pipe(descriptors) == 0);
    assert(close(descriptors[0]) == 0);
    sigset_t blocked, old_mask, observed;
    sigemptyset(&blocked);
    sigaddset(&blocked, SIGUSR2);
    assert(pthread_sigmask(SIG_BLOCK, &blocked, &old_mask) == 0);
    assert(scoop_platform_fd_write(descriptors[1], "x", 1) == -1 && errno == EPIPE);
    assert(signals == 0);
    assert(sigaction(SIGPIPE, NULL, &current) == 0 && current.sa_handler == handler);
    assert(pthread_sigmask(SIG_SETMASK, NULL, &observed) == 0);
    assert(sigismember(&observed, SIGUSR2) == 1);
    assert(sigismember(&observed, SIGPIPE) == sigismember(&old_mask, SIGPIPE));

    sigemptyset(&blocked);
    sigaddset(&blocked, SIGPIPE);
    assert(pthread_sigmask(SIG_BLOCK, &blocked, NULL) == 0);
    assert(raise(SIGPIPE) == 0);
    assert(scoop_platform_fd_write(descriptors[1], "y", 1) == -1 && errno == EPIPE);
    assert(sigpending(&observed) == 0 && sigismember(&observed, SIGPIPE) == 1);
    int consumed;
    assert(sigwait(&blocked, &consumed) == 0 && consumed == SIGPIPE);
    assert(pthread_sigmask(SIG_SETMASK, &old_mask, NULL) == 0);
    assert(signals == 0);
    assert(close(descriptors[1]) == 0);
    assert(sigaction(SIGPIPE, &old_action, NULL) == 0);
}

int main(void) {
    byte_transfers();
    broken_pipe();
    const char *directory = getenv("TMPDIR");
    if (directory == NULL || directory[0] == '\0')
        directory = ".";
    char *temporary = NULL;
    assert(asprintf(&temporary, "%s/scoop-native-io-XXXXXX", directory) > 0);
    assert(scoop_platform_path_temporary(temporary, false) == 0);
    assert(temporary_creates == 1);
    assert(unlink(temporary) == 0);
    free(temporary);
    return 0;
}
