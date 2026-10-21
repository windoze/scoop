#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef int32_t (*Callback)(int32_t, void *);
typedef struct Worker {
    Callback callback;
    void *context;
    int32_t mode;
    atomic_bool returned;
} Worker;

static void *exit_worker(void *raw) {
    Worker *worker = raw;
    worker->callback(worker->mode, worker->context);
    atomic_store_explicit(&worker->returned, 1, memory_order_release);
    return NULL;
}

void blocked_exit(Callback callback, void *context) {
    alarm(20);
    assert(fflush(stdout) == 0);
    int saved = dup(STDOUT_FILENO);
    int pipe_fds[2];
    assert(saved >= 0 && pipe(pipe_fds) == 0);
    assert(fcntl(pipe_fds[1], F_SETFL, O_NONBLOCK) == 0);
    char buffer[4096];
    memset(buffer, 'q', sizeof(buffer));
    while (write(pipe_fds[1], buffer, sizeof(buffer)) > 0) {
    }
    assert(errno == EAGAIN || errno == EWOULDBLOCK);
    assert(fcntl(pipe_fds[1], F_SETFL, 0) == 0);
    assert(dup2(pipe_fds[1], STDOUT_FILENO) == STDOUT_FILENO);
    close(pipe_fds[1]);
    Worker worker = {callback, context, getenv("M33_PANIC") == NULL ? 1 : 2, ATOMIC_VAR_INIT(0)};
    pthread_t thread;
    assert(pthread_create(&thread, NULL, exit_worker, &worker) == 0);
    while (ftrylockfile(stdout) == 0) {
        funlockfile(stdout);
        assert(!atomic_load_explicit(&worker.returned, memory_order_acquire));
        sched_yield();
    }
    assert(callback(0, context) == 37);
    assert(write(saved, "gc-before-exit\n", 15) == 15);
    /* Only now let the flush finish. The whole process exits from the worker. */
    while (read(pipe_fds[0], buffer, sizeof(buffer)) > 0) {
    }
    _exit(98);
}
