#ifndef SCOOP_ASYNCIO_SIGPIPE_H
#define SCOOP_ASYNCIO_SIGPIPE_H

#include <errno.h>
#include <stdbool.h>
#include <stdlib.h>

#ifdef __APPLE__
typedef struct {
    int unused;
} ScoopIoSignalGuard;
static inline void scoop_io_signal_enter(ScoopIoSignalGuard *guard) { (void)guard; }
static inline void scoop_io_signal_leave(ScoopIoSignalGuard *guard) { (void)guard; }
#else
#include <pthread.h>
#include <signal.h>
typedef struct {
    sigset_t previous;
    sigset_t blocked;
    bool pending;
} ScoopIoSignalGuard;

static inline void scoop_io_signal_enter(ScoopIoSignalGuard *guard) {
    sigset_t pending;
    sigemptyset(&guard->blocked);
    sigaddset(&guard->blocked, SIGPIPE);
    /* These valid, local masks cannot fail on the supported POSIX targets. */
    if (pthread_sigmask(SIG_BLOCK, &guard->blocked, &guard->previous) != 0)
        abort();
    if (sigpending(&pending) != 0)
        abort();
    guard->pending = sigismember(&pending, SIGPIPE) == 1;
}

static inline void scoop_io_signal_leave(ScoopIoSignalGuard *guard) {
    int saved = errno;
    if (!guard->pending) {
        const struct timespec zero = {0, 0};
        while (sigtimedwait(&guard->blocked, NULL, &zero) < 0 && errno == EINTR) {
        }
    }
    if (pthread_sigmask(SIG_SETMASK, &guard->previous, NULL) != 0)
        abort();
    errno = saved;
}
#endif

#endif
