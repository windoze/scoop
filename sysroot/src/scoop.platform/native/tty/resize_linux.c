#define _GNU_SOURCE 1
#include "terminal.h"
#include <errno.h>
#include <fcntl.h>
#include <sched.h>
#include <unistd.h>

_Static_assert(ATOMIC_POINTER_LOCK_FREE == 2 && ATOMIC_INT_LOCK_FREE == 2 &&
                   ATOMIC_BOOL_LOCK_FREE == 2,
               "signal notification requires lock-free atomics");
static _Atomic(scoop_terminal *) current;
static atomic_uint handlers;

static void resized(int signal, siginfo_t *info, void *context) {
    int saved = errno;
    atomic_fetch_add(&handlers, 1);
    scoop_terminal *terminal = atomic_load(&current);
    if (terminal) {
        char byte = 1;
        ssize_t sent;
        do {
            sent = write(terminal->notify_write, &byte, 1);
        } while (sent < 0 && errno == EINTR);
        errno = saved;
        const struct sigaction *old = &terminal->previous;
        if (old->sa_handler != SIG_DFL && old->sa_handler != SIG_IGN &&
            (!(old->sa_flags & SA_RESETHAND) ||
             !atomic_exchange(&terminal->previous_called, true))) {
            if (old->sa_flags & SA_SIGINFO)
                old->sa_sigaction(signal, info, context);
            else
                old->sa_handler(signal);
        }
    }
    atomic_fetch_sub(&handlers, 1);
    errno = saved;
}

static void detach_current(void) {
    atomic_store(&current, NULL);
    while (atomic_load(&handlers) != 0)
        sched_yield();
}

static int close_notifications(scoop_terminal *terminal, int error) {
    if (terminal->notify_read >= 0 && close(terminal->notify_read) < 0 && error == 0)
        error = errno;
    if (terminal->notify_write >= 0 && close(terminal->notify_write) < 0 && error == 0)
        error = errno;
    terminal->notify_read = terminal->notify_write = -1;
    terminal->watching = false;
    errno = error;
    return error == 0 ? 0 : -1;
}

int scoop_terminal_watch_start(scoop_terminal *terminal) {
    terminal->notify_read = terminal->notify_write = -1;
    atomic_init(&terminal->previous_called, false);
    int descriptors[2];
    if (pipe(descriptors) < 0)
        return -1;
    terminal->notify_read = descriptors[0];
    terminal->notify_write = descriptors[1];
    for (int index = 0; index < 2; ++index) {
        if (fcntl(descriptors[index], F_SETFD, FD_CLOEXEC) < 0 ||
            fcntl(descriptors[index], F_SETFL, O_NONBLOCK) < 0)
            return close_notifications(terminal, errno);
    }
    if (sigaction(SIGWINCH, NULL, &terminal->previous) < 0)
        return close_notifications(terminal, errno);
    struct sigaction action = terminal->previous;
    action.sa_sigaction = resized;
    action.sa_flags = SA_SIGINFO | (action.sa_flags & (SA_ONSTACK | SA_NODEFER | SA_RESTART));
    if (terminal->previous.sa_flags & SA_RESETHAND)
        action.sa_flags |= SA_NODEFER;
    atomic_store(&current, terminal);
    if (sigaction(SIGWINCH, &action, NULL) < 0) {
        int error = errno;
        detach_current();
        return close_notifications(terminal, error);
    }
    terminal->watching = true;
    return 0;
}

int scoop_terminal_watch_stop(scoop_terminal *terminal) {
    int error = 0;
    detach_current();
    if (terminal->watching) {
        struct sigaction installed;
        if (sigaction(SIGWINCH, NULL, &installed) < 0) {
            error = errno;
        } else if ((installed.sa_flags & SA_SIGINFO) && installed.sa_sigaction == resized) {
            struct sigaction restore = terminal->previous;
            if ((restore.sa_flags & SA_RESETHAND) && atomic_load(&terminal->previous_called)) {
                restore.sa_handler = SIG_DFL;
                restore.sa_flags = 0;
                sigemptyset(&restore.sa_mask);
            }
            if (sigaction(SIGWINCH, &restore, NULL) < 0)
                error = errno;
        }
    }
    return close_notifications(terminal, error);
}

int scoop_terminal_watch_drain(scoop_terminal *terminal) {
    char bytes[64];
    ssize_t count;
    do {
        count = read(terminal->notify_read, bytes, sizeof(bytes));
    } while (count > 0 || (count < 0 && errno == EINTR));
    return count < 0 && errno != EAGAIN && errno != EWOULDBLOCK ? -1 : 0;
}
