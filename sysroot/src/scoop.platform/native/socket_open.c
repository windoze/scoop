#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "socket_native.h"
#ifdef _WIN32

static INIT_ONCE winsock_once = INIT_ONCE_STATIC_INIT;
static int winsock_error;

static BOOL CALLBACK initialize_winsock(INIT_ONCE *once, void *argument, void **context) {
    (void)once;
    (void)argument;
    (void)context;
    WSADATA data;
    winsock_error = WSAStartup(MAKEWORD(2, 2), &data);
    return TRUE;
}

int scoop_socket_startup(void) {
    (void)InitOnceExecuteOnce(&winsock_once, initialize_winsock, NULL, NULL);
    return winsock_error;
}

static SOCKET socket_failed(SOCKET socket) {
    int error = WSAGetLastError();
    (void)closesocket(socket);
    WSASetLastError(error);
    return INVALID_SOCKET;
}

static SOCKET create_socket(int family, int type) {
    int error = scoop_socket_startup();
    if (error != 0) {
        WSASetLastError(error);
        return INVALID_SOCKET;
    }
    return WSASocketW(family == 4 ? AF_INET : AF_INET6, type, 0, NULL, 0,
                      WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT);
}

static SOCKET connect_socket(struct ScoopSocketAddress address) {
    struct sockaddr_storage native;
    int length = scoop_socket_address_encode(address, &native);
    if (length == 0)
        return INVALID_SOCKET;
    SOCKET socket = create_socket(address.family, SOCK_STREAM);
    if (socket == INVALID_SOCKET)
        return socket;
    return connect(socket, (const struct sockaddr *)&native, length) == 0 ? socket
                                                                          : socket_failed(socket);
}

static SOCKET bind_socket(struct ScoopSocketAddress address, bool datagram, bool reuse,
                          int backlog) {
    struct sockaddr_storage native;
    int length = scoop_socket_address_encode(address, &native);
    if (length == 0)
        return INVALID_SOCKET;
    SOCKET socket = create_socket(address.family, datagram ? SOCK_DGRAM : SOCK_STREAM);
    if (socket == INVALID_SOCKET)
        return socket;
    BOOL enabled = reuse;
    if (setsockopt(socket, SOL_SOCKET, SO_REUSEADDR, (const char *)&enabled, sizeof(enabled)) !=
            0 ||
        bind(socket, (const struct sockaddr *)&native, length) != 0 ||
        (!datagram && listen(socket, backlog) != 0))
        return socket_failed(socket);
    return socket;
}

static SOCKET accept_socket(SOCKET listener) { return accept(listener, NULL, NULL); }

static int socket_option(SOCKET socket, bool no_delay, bool set, bool value) {
    int level = no_delay ? IPPROTO_TCP : SOL_SOCKET;
    int option = no_delay ? TCP_NODELAY : SO_KEEPALIVE;
    BOOL enabled = value;
    if (set)
        return setsockopt(socket, level, option, (const char *)&enabled, sizeof(enabled));
    int length = sizeof(enabled);
    return getsockopt(socket, level, option, (char *)&enabled, &length) == 0 ? enabled != 0 : -1;
}

static int shutdown_socket(SOCKET socket, bool output) {
    return shutdown(socket, output ? SD_SEND : SD_RECEIVE);
}
#else
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

static int connect_socket(struct ScoopSocketAddress address) {
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

static int accept_socket(int listener) {
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

static int socket_option(int descriptor, bool no_delay, bool set, bool value) {
    int level = no_delay ? IPPROTO_TCP : SOL_SOCKET;
    int option = no_delay ? TCP_NODELAY : SO_KEEPALIVE;
    int enabled = value;
    if (set)
        return setsockopt(descriptor, level, option, &enabled, sizeof(enabled));
    socklen_t length = sizeof(enabled);
    return getsockopt(descriptor, level, option, &enabled, &length) < 0 ? -1 : enabled != 0;
}

static int shutdown_socket(int descriptor, bool output) {
    return shutdown(descriptor, output ? SHUT_WR : SHUT_RD);
}

#define bind_socket scoop_platform_socket_bind
#endif

static struct ScoopSocketOpened opened(ScoopOsSocket socket) {
    return socket == SCOOP_INVALID_SOCKET
               ? (struct ScoopSocketOpened){{UINT64_MAX}, scoop_socket_error()}
               : (struct ScoopSocketOpened){{(uint64_t)socket}, 0};
}

struct ScoopSocketOpened scoop_platform_tcp_connect(struct ScoopSocketAddress address) {
    return opened(connect_socket(address));
}

struct ScoopSocketOpened scoop_platform_socket_bind_owned(struct ScoopSocketAddress address,
                                                          bool datagram, bool reuse, int backlog) {
    return opened(bind_socket(address, datagram, reuse, backlog));
}

struct ScoopSocketOpened scoop_platform_tcp_accept(struct ScoopSocketHandle listener) {
    return opened(accept_socket((ScoopOsSocket)listener.value));
}

struct ScoopSocketResult scoop_platform_socket_option(struct ScoopSocketHandle socket,
                                                      bool no_delay, bool set, bool value) {
    int result = socket_option((ScoopOsSocket)socket.value, no_delay, set, value);
    return (struct ScoopSocketResult){result, result < 0 ? scoop_socket_error() : 0};
}

struct ScoopSocketResult scoop_platform_tcp_shutdown(struct ScoopSocketHandle socket, bool output) {
    int result = shutdown_socket((ScoopOsSocket)socket.value, output);
    return (struct ScoopSocketResult){result, result < 0 ? scoop_socket_error() : 0};
}

struct ScoopSocketResult scoop_platform_socket_close(struct ScoopSocketHandle socket) {
#ifdef _WIN32
    int result = closesocket((SOCKET)socket.value);
#else
    int result = close((int)socket.value);
#endif
    return (struct ScoopSocketResult){result, result < 0 ? scoop_socket_error() : 0};
}
