#define _GNU_SOURCE 1
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <spawn.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

typedef int32_t (*Callback)(void *);
static Callback callback;
static void *context;
static _Atomic int flags, tracked[64], waits, signals;
static pid_t last_child;
static sigset_t previous_mask;
static struct sigaction previous_action;

void m45_process_signal_setup(void) {
    sigset_t mask;
    sigemptyset(&mask);
    sigaddset(&mask, SIGPIPE);
    assert(pthread_sigmask(SIG_BLOCK, &mask, &previous_mask) == 0);
    struct sigaction action = {0};
    action.sa_handler = SIG_IGN;
    sigemptyset(&action.sa_mask);
    assert(sigaction(SIGPIPE, &action, &previous_action) == 0);
}

void m45_process_signal_restore(void) {
    sigset_t mask;
    assert(pthread_sigmask(SIG_SETMASK, NULL, &mask) == 0 && sigismember(&mask, SIGPIPE));
    struct sigaction action;
    assert(sigaction(SIGPIPE, NULL, &action) == 0 && action.sa_handler == SIG_IGN);
    assert(sigaction(SIGPIPE, &previous_action, NULL) == 0);
    assert(pthread_sigmask(SIG_SETMASK, &previous_mask, NULL) == 0);
}

void m45_process_callback(Callback function, void *cookie) {
    callback = function;
    context = cookie;
}
void m45_process_clear_callback(void) {
    callback = NULL;
    context = NULL;
}
static void collect(void) {
    if (callback != NULL)
        assert(callback(context) == 0);
}
void m45_process_fail(int value) { atomic_store(&flags, value); }
static int take(int flag) { return (atomic_fetch_and(&flags, ~flag) & flag) != 0; }

static void record(int fd) {
    for (int index = 0; index < 64; ++index) {
        int empty = 0;
        if (atomic_compare_exchange_strong(&tracked[index], &empty, fd + 1))
            return;
    }
    abort();
}

int m45_process_descriptors(void) {
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
int m45_process_waits(void) { return atomic_load(&waits); }
int m45_process_signals(void) { return atomic_load(&signals); }

#ifdef __APPLE__
int m45_process_pipe(int descriptors[2]) {
    int result = pipe(descriptors);
#else
int m45_process_pipe2(int descriptors[2], int options) {
    int result = pipe2(descriptors, options);
#endif
    if (result == 0) {
        record(descriptors[0]);
        record(descriptors[1]);
    }
    return result;
}

int m45_process_close(int fd) {
    int owned = 0;
    for (int index = 0; index < 64; ++index) {
        int value = fd + 1;
        if (atomic_compare_exchange_strong(&tracked[index], &value, 0))
            owned = 1;
    }
    int result = close(fd);
    if (owned && take(8)) {
        assert(result == 0);
        errno = EINTR;
        return -1;
    }
    return result;
}

void *m45_process_calloc(size_t count, size_t size) {
    if (take(1)) {
        errno = ENOMEM;
        return NULL;
    }
    return calloc(count, size);
}

int m45_process_dup2(posix_spawn_file_actions_t *actions, int from, int to) {
    if (take(2))
        return EMFILE;
    return posix_spawn_file_actions_adddup2(actions, from, to);
}

int m45_process_spawn(pid_t *child, const char *path, const posix_spawn_file_actions_t *actions,
                      const posix_spawnattr_t *attributes, char *const argv[], char *const envp[]) {
    collect();
    if (take(4))
        return EACCES;
    int error = posix_spawn(child, path, actions, attributes, argv, envp);
    if (error == 0) {
        last_child = *child;
        if (take(256)) {
            int status;
            assert(waitpid(*child, &status, 0) == *child);
        }
    }
    return error;
}

pid_t m45_process_waitpid(pid_t child, int *status, int options) {
    atomic_fetch_add(&waits, 1);
    if (options == 0) {
        if (take(16)) {
            errno = EINTR;
            return -1;
        }
        collect();
    }
    return waitpid(child, status, options);
}

int m45_process_kill(pid_t child, int signal) {
    int before = atomic_load(&waits);
    collect();
    assert(atomic_load(&waits) == before);
    atomic_fetch_add(&signals, 1);
    return kill(child, signal);
}

int m45_process_poll(struct pollfd *descriptors, nfds_t count, int timeout) {
    if (take(32)) {
        errno = EINTR;
        return -1;
    }
    collect();
    return poll(descriptors, count, timeout);
}
ssize_t m45_process_read(int fd, void *bytes, size_t count) {
    if (take(64)) {
        errno = EAGAIN;
        return -1;
    }
    collect();
    return read(fd, bytes, count);
}
ssize_t m45_process_write(int fd, const void *bytes, size_t count) {
    if (take(128)) {
        errno = EAGAIN;
        return -1;
    }
    collect();
    return write(fd, bytes, count);
}

void m45_process_exited(void) {
    siginfo_t info;
    assert(waitid(P_PID, (id_t)last_child, &info, WEXITED | WNOWAIT) == 0);
}
void m45_process_reaped(void) { assert(waitpid(last_child, NULL, WNOHANG) < 0 && errno == ECHILD); }
void m45_process_stopped(void) {
    int status;
    assert(waitpid(last_child, &status, WUNTRACED) == last_child && WIFSTOPPED(status));
}
void m45_process_cleanup_stopped(void) {
    assert(kill(last_child, SIGKILL) == 0);
    int status;
    assert(waitpid(last_child, &status, 0) == last_child && WIFSIGNALED(status));
}

void m45_process_cleanup_signaled(void) {
    int status;
    pid_t result = waitpid(last_child, &status, 0);
    assert(result == last_child || (result < 0 && errno == ECHILD));
}
