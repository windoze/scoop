#define _GNU_SOURCE 1
#include "socket_native.h"
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <unistd.h>

int scoop_socket_failed(int descriptor) {
    int error = errno;
    (void)close(descriptor);
    errno = error;
    return -1;
}

int scoop_socket_prepare(int descriptor) {
#ifdef __APPLE__
    int enabled = 1;
    if (fcntl(descriptor, F_SETFD, FD_CLOEXEC) < 0 ||
        setsockopt(descriptor, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled)) < 0)
        return scoop_socket_failed(descriptor);
#endif
    return descriptor;
}

static int create_socket(int family, int type) {
#ifndef __APPLE__
    type |= SOCK_CLOEXEC;
#endif
    int descriptor = socket(family == 4 ? AF_INET : AF_INET6, type, 0);
    return descriptor < 0 ? -1 : scoop_socket_prepare(descriptor);
}

int scoop_platform_tcp_connect(struct ScoopSocketAddress address) {
    struct sockaddr_storage native;
    socklen_t length = scoop_socket_address_encode(address, &native);
    if (length == 0)
        return -1;
    int descriptor = create_socket(address.family, SOCK_STREAM);
    if (descriptor < 0)
        return -1;
    if (connect(descriptor, (const struct sockaddr *)&native, length) < 0)
        return scoop_socket_failed(descriptor);
    return descriptor;
}

int scoop_platform_socket_bind(struct ScoopSocketAddress address, bool datagram, bool reuse,
                               int backlog) {
    struct sockaddr_storage native;
    socklen_t length = scoop_socket_address_encode(address, &native);
    if (length == 0)
        return -1;
    int descriptor = create_socket(address.family, datagram ? SOCK_DGRAM : SOCK_STREAM);
    if (descriptor < 0)
        return -1;
    int enabled = reuse;
    if (setsockopt(descriptor, SOL_SOCKET, SO_REUSEADDR, &enabled, sizeof(enabled)) < 0 ||
        bind(descriptor, (const struct sockaddr *)&native, length) < 0 ||
        (!datagram && listen(descriptor, backlog) < 0))
        return scoop_socket_failed(descriptor);
    return descriptor;
}

int scoop_platform_tcp_accept(int listener) {
    int descriptor;
    do {
#ifdef __APPLE__
        descriptor = accept(listener, NULL, NULL);
#else
        descriptor = accept4(listener, NULL, NULL, SOCK_CLOEXEC);
#endif
    } while (descriptor < 0 && errno == EINTR);
    return descriptor < 0 ? -1 : scoop_socket_prepare(descriptor);
}

int scoop_platform_socket_option(int descriptor, bool no_delay, bool set, bool value) {
    int level = no_delay ? IPPROTO_TCP : SOL_SOCKET;
    int option = no_delay ? TCP_NODELAY : SO_KEEPALIVE;
    int enabled = value;
    if (set)
        return setsockopt(descriptor, level, option, &enabled, sizeof(enabled));
    socklen_t length = sizeof(enabled);
    return getsockopt(descriptor, level, option, &enabled, &length) < 0 ? -1 : enabled != 0;
}

int scoop_platform_tcp_shutdown(int descriptor, bool output) {
    return shutdown(descriptor, output ? SHUT_WR : SHUT_RD);
}
