#ifdef __APPLE__
#include "../../native/darwin/endpoint.h"
#else
#include "../../native/tcp.h"
#endif
#include <errno.h>
#include <poll.h>

static int socket_fd(void *pointer) {
    int fd;
#ifdef __APPLE__
    fd = ((ScoopIoEndpoint *)pointer)->fd;
#else
    int status = uv_fileno((uv_handle_t *)&((ScoopIoTcp *)pointer)->handle, &fd);
    if (status < 0)
        return status;
#endif
    return fd;
}

int scoop_test_socket_break_output(void *pointer) {
    int fd = socket_fd(pointer);
    if (fd < 0)
        return fd;
    return shutdown(fd, SHUT_WR);
}

int scoop_test_socket_output_blocked(void *pointer) {
    int fd = socket_fd(pointer);
    if (fd < 0)
        return fd;
    struct pollfd wait = {.fd = fd, .events = POLLOUT};
    int result;
    do {
        result = poll(&wait, 1, 0);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return -errno;
    if (wait.revents & (POLLERR | POLLHUP | POLLNVAL))
        return -EIO;
    return !(wait.revents & POLLOUT);
}
