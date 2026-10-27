#define _GNU_SOURCE 1
#include "terminal.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <poll.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <unistd.h>

static atomic_bool session_open;

bool scoop_terminal_is_tty(int stream) { return isatty(stream) == 1; }

void *scoop_terminal_open(void) {
    bool expected = false;
    if (!atomic_compare_exchange_strong(&session_open, &expected, true)) {
        errno = EBUSY;
        return NULL;
    }
    char path[PATH_MAX];
    const char *selected = "/dev/tty";
    int fd = -1;
    scoop_terminal *terminal = NULL;
    if (isatty(STDIN_FILENO)) {
        int error = ttyname_r(STDIN_FILENO, path, sizeof(path));
        if (error != 0) {
            errno = error;
            goto failed;
        }
        selected = path;
    }
    do {
        fd = open(selected, O_RDWR | O_CLOEXEC | O_NONBLOCK | O_NOCTTY);
    } while (fd < 0 && errno == EINTR);
    if (fd < 0)
        goto failed;
    terminal = calloc(1, sizeof(*terminal));
    if (!terminal)
        goto failed;
    terminal->fd = fd;
    if (scoop_terminal_watch_start(terminal) < 0)
        goto failed;
    return terminal;
failed: {
    int error = errno;
    if (fd >= 0)
        (void)close(fd);
    free(terminal);
    atomic_store(&session_open, false);
    errno = error;
    return NULL;
}
}

int scoop_terminal_close(scoop_terminal *terminal) {
    int error = 0;
    if (scoop_terminal_watch_stop(terminal) < 0)
        error = errno;
    if (scoop_terminal_set_raw(terminal, false) < 0 && error == 0)
        error = errno;
    if (close(terminal->fd) < 0 && error == 0)
        error = errno;
    free(terminal);
    atomic_store(&session_open, false);
    errno = error;
    return error == 0 ? 0 : -1;
}

int64_t scoop_terminal_try_write(scoop_terminal *terminal, const void *data, int64_t count) {
    if (count == 0)
        return 0;
    size_t length = count > INT_MAX ? INT_MAX : (size_t)count;
    ssize_t result;
    do {
        result = write(terminal->fd, data, length);
    } while (result < 0 && errno == EINTR);
    return result;
}

int64_t scoop_terminal_write(scoop_terminal *terminal, const void *data, int64_t count) {
    for (;;) {
        int64_t result = scoop_terminal_try_write(terminal, data, count);
        if (result >= 0)
            return result;
        if (errno != EAGAIN && errno != EWOULDBLOCK)
            return -1;
        struct pollfd wait = {.fd = terminal->fd, .events = POLLOUT};
        int ready;
        do {
            ready = poll(&wait, 1, -1);
        } while (ready < 0 && errno == EINTR);
        if (ready < 0)
            return -1;
    }
}
