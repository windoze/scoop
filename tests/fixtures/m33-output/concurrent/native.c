#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sched.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

typedef int32_t (*Callback)(int32_t, void *);
typedef struct Writer {
    Callback callback;
    void *context;
    int32_t mode;
    int32_t result;
    atomic_bool done;
} Writer;

static void *write_output(void *raw) {
    Writer *writer = raw;
    writer->result = writer->callback(writer->mode, writer->context);
    atomic_store_explicit(&writer->done, 1, memory_order_release);
    return NULL;
}

int32_t blocked_output(Callback callback, void *context, int32_t mode, int64_t length) {
    alarm(20);
    FILE *stream = mode == 1 ? stderr : stdout;
    int target = mode == 1 ? STDERR_FILENO : STDOUT_FILENO;
    assert(fflush(stream) == 0);
    int saved = dup(target);
    int pipe_fds[2];
    assert(saved >= 0 && pipe(pipe_fds) == 0);
    assert(fcntl(pipe_fds[1], F_SETFL, O_NONBLOCK) == 0);
    char buffer[4096];
    memset(buffer, 'q', sizeof(buffer));
    size_t filled = 0;
    for (;;) {
        ssize_t count = write(pipe_fds[1], buffer, sizeof(buffer));
        if (count < 0) {
            assert(errno == EAGAIN || errno == EWOULDBLOCK);
            break;
        }
        filled += (size_t)count;
    }
    assert(fcntl(pipe_fds[1], F_SETFL, 0) == 0);
    assert(dup2(pipe_fds[1], target) == target);
    close(pipe_fds[1]);
    Writer writer = {callback, context, mode, 0, ATOMIC_VAR_INIT(0)};
    pthread_t thread;
    assert(pthread_create(&thread, NULL, write_output, &writer) == 0);
    /* A full pipe keeps the writer inside stdio with its stream lock held. */
    while (ftrylockfile(stream) == 0) {
        funlockfile(stream);
        assert(!atomic_load_explicit(&writer.done, memory_order_acquire));
        sched_yield();
    }
    /* The callback must finish GC before any bytes can leave the pipe. */
    assert(callback(-1, context) == 37);
    size_t received = 0;
    while (received < filled + (size_t)length) {
        ssize_t count = read(pipe_fds[0], buffer, sizeof(buffer));
        assert(count > 0);
        for (ssize_t index = 0; index < count; index++) {
            size_t position = received + (size_t)index;
            char expected = position < filled ? 'q'
                            : mode == 2       ? "pending"[position - filled]
                                              : 'x';
            assert(buffer[index] == expected);
        }
        received += (size_t)count;
    }
    assert(pthread_join(thread, NULL) == 0 && writer.result == 37);
    assert(dup2(saved, target) == target);
    close(saved);
    close(pipe_fds[0]);
    alarm(0);
    return 1;
}
