#define _GNU_SOURCE 1
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdint.h>
#include <sys/socket.h>
#include <unistd.h>

typedef int32_t (*Callback)(void *);
static Callback callback;
static void *context;
static _Atomic int flags, tracked[64], resolutions;
static volatile sig_atomic_t received_signal;
static struct sigaction previous_action;
extern void m45_network_start_peer(int);
extern void m45_network_finish_peer(void);
extern void m45_network_send_packet(int);

void m45_network_callback(Callback function, void *cookie) {
    callback = function;
    context = cookie;
}
void m45_network_clear_callback(void) {
    callback = NULL;
    context = NULL;
}
static void collect(void) {
    if (callback != NULL)
        assert(callback(context) == 0);
}
void m45_network_flags(int value) { atomic_store(&flags, value); }
static int take(int bit) { return (atomic_fetch_and(&flags, ~bit) & bit) != 0; }
int m45_network_resolutions(void) { return atomic_load(&resolutions); }
int m45_network_eai_system(void) { return EAI_SYSTEM; }
int m45_network_eai_noname(void) { return EAI_NONAME; }

static void record(int fd) {
    for (int index = 0; index < 64; ++index) {
        int empty = 0;
        if (atomic_compare_exchange_strong(&tracked[index], &empty, fd + 1))
            return;
    }
    assert(0 && "too many live fixture sockets");
}
int m45_network_descriptors(void) {
    int count = 0;
    for (int index = 0; index < 64; ++index) {
        int fd = atomic_load(&tracked[index]) - 1;
        if (fd >= 0) {
            assert(fcntl(fd, F_GETFD) == FD_CLOEXEC);
            ++count;
        }
    }
    return count;
}

int m45_network_socket(int family, int type, int protocol) {
    if (take(1)) {
        errno = ENFILE;
        return -1;
    }
    int descriptor = socket(family, type, protocol);
    if (descriptor >= 0)
        record(descriptor);
    return descriptor;
}
int m45_network_close(int descriptor) {
    int owned = 0;
    for (int index = 0; index < 64; ++index) {
        int value = descriptor + 1;
        if (atomic_compare_exchange_strong(&tracked[index], &value, 0))
            owned = 1;
    }
    int result = close(descriptor);
    if (owned && take(4096)) {
        assert(result == 0);
        errno = EINTR;
        return -1;
    }
    return result;
}
int m45_network_setsockopt(int descriptor, int level, int option, const void *value,
                           socklen_t length) {
    if (take(2)) {
        errno = ENOMEM;
        return -1;
    }
    return setsockopt(descriptor, level, option, value, length);
}
int m45_network_bind(int descriptor, const struct sockaddr *address, socklen_t length) {
    if (take(4)) {
        errno = EADDRINUSE;
        return -1;
    }
    return bind(descriptor, address, length);
}
int m45_network_connect(int descriptor, const struct sockaddr *address, socklen_t length) {
    if (take(8192)) {
        errno = ECONNREFUSED;
        return -1;
    }
    return connect(descriptor, address, length);
}

#ifdef __APPLE__
int m45_network_accept(int listener, struct sockaddr *address, socklen_t *length) {
#else
int m45_network_accept4(int listener, struct sockaddr *address, socklen_t *length, int options) {
#endif
    if (take(64)) {
        errno = EINTR;
        return -1;
    }
    collect();
    assert(fcntl(listener, F_GETFD) == FD_CLOEXEC);
    int automatic = take(128);
    if (automatic)
        m45_network_start_peer(listener);
#ifdef __APPLE__
    int result = accept(listener, address, length);
#else
    int result = accept4(listener, address, length, options);
#endif
    if (automatic)
        m45_network_finish_peer();
    if (result >= 0)
        record(result);
    return result;
}

static void check_send(int descriptor, int options) {
    collect();
    assert(fcntl(descriptor, F_GETFD) == FD_CLOEXEC);
#ifdef __APPLE__
    (void)options;
    int enabled = 0;
    socklen_t length = sizeof(enabled);
    assert(getsockopt(descriptor, SOL_SOCKET, SO_NOSIGPIPE, &enabled, &length) == 0 && enabled);
#else
    assert((options & MSG_NOSIGNAL) != 0);
#endif
}
ssize_t m45_network_send(int descriptor, const void *bytes, size_t count, int options) {
    if (take(8)) {
        errno = EINTR;
        return -1;
    }
    check_send(descriptor, options);
    if ((atomic_load(&flags) & 2048) && count > 3)
        count = 3;
    return send(descriptor, bytes, count, options);
}
ssize_t m45_network_sendto(int descriptor, const void *bytes, size_t count, int options,
                           const struct sockaddr *address, socklen_t length) {
    if (take(8)) {
        errno = EINTR;
        return -1;
    }
    check_send(descriptor, options);
    return sendto(descriptor, bytes, count, options, address, length);
}
ssize_t m45_network_recvmsg(int descriptor, struct msghdr *message, int options) {
    if (take(16)) {
        errno = EINTR;
        return -1;
    }
    collect();
    assert(fcntl(descriptor, F_GETFD) == FD_CLOEXEC);
    if (take(256))
        m45_network_send_packet(descriptor);
    return recvmsg(descriptor, message, options);
}
ssize_t m45_network_read(int descriptor, void *bytes, size_t count) {
    if (take(32)) {
        errno = EINTR;
        return -1;
    }
    collect();
    assert(fcntl(descriptor, F_GETFD) == FD_CLOEXEC);
    return read(descriptor, bytes, count);
}

int m45_network_getaddrinfo(const char *host, const char *service, const struct addrinfo *hints,
                            struct addrinfo **result) {
    collect();
    if (take(512)) {
        errno = EACCES;
        return EAI_SYSTEM;
    }
    if (take(1024)) {
        errno = EIO;
        return EAI_NONAME;
    }
    return getaddrinfo(host, service, hints, result);
}
void m45_network_freeaddrinfo(struct addrinfo *head) {
    collect();
    atomic_fetch_add(&resolutions, 1);
    freeaddrinfo(head);
}

static void signal_handler(int signal) {
    (void)signal;
    received_signal = 1;
}
void m45_network_signal_setup(void) {
    struct sigaction action = {0};
    action.sa_handler = signal_handler;
    sigemptyset(&action.sa_mask);
    assert(sigaction(SIGPIPE, &action, &previous_action) == 0);
}
void m45_network_signal_restore(void) {
    struct sigaction action;
    assert(sigaction(SIGPIPE, NULL, &action) == 0 && action.sa_handler == signal_handler);
    assert(!received_signal);
    assert(sigaction(SIGPIPE, &previous_action, NULL) == 0);
}
