#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "socket_native.h"
#include <errno.h>
#include <limits.h>

#if defined(__APPLE__) || defined(_WIN32)
#define SEND_FLAGS 0
#else
#define SEND_FLAGS MSG_NOSIGNAL
#endif

struct ScoopSocketResult scoop_platform_tcp_write(struct ScoopSocketHandle socket,
                                                  const void *bytes, int64_t count) {
#ifdef _WIN32
    int length = (int)(count < INT_MAX ? count : INT_MAX);
    int64_t written = send((SOCKET)socket.value, bytes, length, 0);
#else
    size_t length = (size_t)(count < INT_MAX ? count : INT_MAX);
    ssize_t written;
    do {
        written = send((int)socket.value, bytes, length, SEND_FLAGS);
    } while (written < 0 && errno == EINTR);
#endif
    return (struct ScoopSocketResult){written, written < 0 ? scoop_socket_error() : 0};
}

struct ScoopSocketResult scoop_platform_tcp_read(struct ScoopSocketHandle socket, void *bytes,
                                                 int64_t count) {
#ifdef _WIN32
    int length = (int)(count < INT_MAX ? count : INT_MAX);
    int64_t received = recv((SOCKET)socket.value, bytes, length, 0);
#else
    size_t length = (size_t)(count < INT_MAX ? count : INT_MAX);
    ssize_t received;
    do {
        received = recv((int)socket.value, bytes, length, 0);
    } while (received < 0 && errno == EINTR);
#endif
    return (struct ScoopSocketResult){received, received < 0 ? scoop_socket_error() : 0};
}

struct ScoopSocketResult scoop_platform_udp_send(struct ScoopSocketHandle socket, const void *bytes,
                                                 int64_t count,
                                                 struct ScoopSocketAddress destination) {
    struct sockaddr_storage address;
    ScoopSocketLength length = scoop_socket_address_encode(destination, &address);
    if (length == 0)
        return (struct ScoopSocketResult){-1, scoop_socket_error()};
#ifdef _WIN32
    if (count > INT_MAX)
        return (struct ScoopSocketResult){-1, WSAEMSGSIZE};
    int64_t sent = sendto((SOCKET)socket.value, bytes != NULL ? bytes : "", (int)count, 0,
                          (struct sockaddr *)&address, length);
#else
    ssize_t sent;
    do {
        sent = sendto((int)socket.value, bytes, (size_t)count, SEND_FLAGS,
                      (struct sockaddr *)&address, length);
    } while (sent < 0 && errno == EINTR);
#endif
    return (struct ScoopSocketResult){sent, sent < 0 ? scoop_socket_error() : 0};
}

struct ScoopSocketReceived scoop_platform_udp_receive(struct ScoopSocketHandle socket, void *bytes,
                                                      int64_t count) {
    struct sockaddr_storage address;
    struct ScoopSocketReceived result = {0};
#ifdef _WIN32
    WSABUF buffer = {(ULONG)((uint64_t)count > UINT32_MAX ? UINT32_MAX : count), bytes};
    DWORD received = 0, flags = 0;
    int length = sizeof(address);
    int status = WSARecvFrom((SOCKET)socket.value, &buffer, 1, &received, &flags,
                             (struct sockaddr *)&address, &length, NULL, NULL);
    int error = status == 0 ? 0 : WSAGetLastError();
    if (error != 0 && error != WSAEMSGSIZE) {
        result.error = error;
        return result;
    }
    result.packet.count = received;
    result.packet.truncated = error == WSAEMSGSIZE;
    result.packet.source = scoop_socket_address_decode((struct sockaddr *)&address, length);
#else
    struct iovec vector = {bytes, (size_t)count};
    struct msghdr message = {0};
    message.msg_name = &address;
    message.msg_iov = &vector;
    message.msg_iovlen = 1;
    do {
        message.msg_namelen = sizeof(address);
        message.msg_flags = 0;
        result.packet.count = recvmsg((int)socket.value, &message, 0);
    } while (result.packet.count < 0 && errno == EINTR);
    if (result.packet.count < 0) {
        result.error = errno;
        return result;
    }
    result.packet.source =
        scoop_socket_address_decode((struct sockaddr *)&address, message.msg_namelen);
    result.packet.truncated = (message.msg_flags & MSG_TRUNC) != 0;
#endif
    if (result.packet.source.family == 0)
        result.error = scoop_socket_error();
    return result;
}
