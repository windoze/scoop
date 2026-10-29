#ifdef _WIN32
#include "../../native/windows/socket.h"

int scoop_test_socket_break_output(ScoopIoSocket *socket) {
    return shutdown(socket->socket, SD_SEND);
}

int scoop_test_socket_output_blocked(ScoopIoSocket *socket) {
    WSAPOLLFD wait = {.fd = socket->socket, .events = POLLOUT};
    if (WSAPoll(&wait, 1, 0) < 0)
        return -WSAGetLastError();
    if (wait.revents & (POLLERR | POLLHUP | POLLNVAL))
        return -WSAECONNRESET;
    return !(wait.revents & POLLOUT);
}
#else
#ifdef __APPLE__
#include "../../native/darwin/endpoint.h"
#else
#include "../../native/linux/endpoint.h"
#endif
#include <errno.h>
#include <poll.h>

static int socket_fd(void *pointer) { return ((ScoopIoEndpoint *)pointer)->fd; }

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
#endif
