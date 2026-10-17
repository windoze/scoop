#define _GNU_SOURCE 1
#include "process_native.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <unistd.h>

int scoop_platform_pipe_open(int32_t descriptors[2]) {
    int pipes[2];
#ifdef __APPLE__
    if (pipe(pipes) < 0)
        return -1;
#else
    if (pipe2(pipes, O_CLOEXEC) < 0)
        return -1;
#endif
    for (int index = 0; index < 2; ++index) {
        if (pipes[index] < 3) {
            int copy = fcntl(pipes[index], F_DUPFD_CLOEXEC, 3);
            if (copy < 0)
                goto fail;
            (void)close(pipes[index]);
            pipes[index] = copy;
        }
#ifdef __APPLE__
        if (fcntl(pipes[index], F_SETFD, FD_CLOEXEC) < 0)
            goto fail;
#endif
    }
    descriptors[0] = pipes[0];
    descriptors[1] = pipes[1];
    return 0;
fail:;
    int error = errno;
    (void)close(pipes[0]);
    (void)close(pipes[1]);
    errno = error;
    return -1;
}

int scoop_platform_pipe_nonblocking(int descriptor) {
    int flags = fcntl(descriptor, F_GETFL);
    return flags < 0 ? -1 : fcntl(descriptor, F_SETFL, flags | O_NONBLOCK);
}

int scoop_platform_process_poll(int input, int output, int error) {
    struct pollfd descriptors[3] = {{input, POLLOUT, 0}, {output, POLLIN, 0}, {error, POLLIN, 0}};
    int result;
    do {
        result = poll(descriptors, 3, -1);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return -1;
    int ready = 0;
    for (int index = 0; index < 3; ++index) {
        if (descriptors[index].revents & POLLNVAL) {
            errno = EBADF;
            return -1;
        }
        if (descriptors[index].revents != 0)
            ready |= 1 << index;
    }
    return ready;
}
