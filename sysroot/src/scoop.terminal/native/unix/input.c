#define _GNU_SOURCE 1
#include "terminal.h"
#include <errno.h>
#include <poll.h>
#include <unistd.h>

int scoop_terminal_wait(scoop_terminal *terminal, int milliseconds) {
    struct pollfd descriptors[2] = {
        {.fd = terminal->fd, .events = POLLIN},
        {.fd = terminal->notify_read, .events = POLLIN},
    };
    int result = poll(descriptors, 2, milliseconds);
    if (result <= 0)
        return result;
    if ((descriptors[0].revents | descriptors[1].revents) & POLLNVAL) {
        errno = EBADF;
        return -1;
    }
    int ready = (descriptors[0].revents & POLLIN) ? 1 : 0;
    if (descriptors[1].revents & POLLIN) {
        if (scoop_terminal_watch_drain(terminal) < 0)
            return -1;
        ready |= 2;
    }
    if (descriptors[0].revents & (POLLHUP | POLLERR))
        ready |= 4;
    return ready;
}

int64_t scoop_terminal_read(scoop_terminal *terminal, void *bytes, int64_t count, bool hung_up) {
    ssize_t result = read(terminal->fd, bytes, (size_t)count);
    if (result < 0 && hung_up && (errno == EIO || errno == EAGAIN || errno == EWOULDBLOCK))
        return 0;
    return result;
}
