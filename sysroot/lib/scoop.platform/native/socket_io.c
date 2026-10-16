#define _GNU_SOURCE 1
#include "socket_native.h"
#include <errno.h>
#include <limits.h>

#ifdef __APPLE__
#define SEND_FLAGS 0
#else
#define SEND_FLAGS MSG_NOSIGNAL
#endif

int64_t scoop_platform_tcp_write(int descriptor, const void *bytes, int64_t count) {
    size_t length = (size_t)(count < INT_MAX ? count : INT_MAX);
    ssize_t written;
    do {
        written = send(descriptor, bytes, length, SEND_FLAGS);
    } while (written < 0 && errno == EINTR);
    return written;
}

int64_t scoop_platform_udp_send(int descriptor, const void *bytes, int64_t count,
                                struct ScoopSocketAddress destination) {
    struct sockaddr_storage address;
    socklen_t length = scoop_socket_address_encode(destination, &address);
    if (length == 0)
        return -1;
    ssize_t sent;
    do {
        sent = sendto(descriptor, bytes, (size_t)count, SEND_FLAGS, (struct sockaddr *)&address,
                      length);
    } while (sent < 0 && errno == EINTR);
    return sent;
}

struct ScoopDatagram scoop_platform_udp_receive(int descriptor, void *bytes, int64_t count) {
    struct sockaddr_storage address;
    struct iovec vector = {bytes, (size_t)count};
    struct msghdr message = {0};
    message.msg_name = &address;
    message.msg_iov = &vector;
    message.msg_iovlen = 1;
    struct ScoopDatagram result = {0};
    do {
        message.msg_namelen = sizeof(address);
        message.msg_flags = 0;
        result.count = recvmsg(descriptor, &message, 0);
    } while (result.count < 0 && errno == EINTR);
    if (result.count >= 0) {
        result.source =
            scoop_socket_address_decode((struct sockaddr *)&address, message.msg_namelen);
        result.truncated = (message.msg_flags & MSG_TRUNC) != 0;
    }
    return result;
}
