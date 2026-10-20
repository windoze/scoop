#include "terminal.h"
#include <errno.h>
#include <fcntl.h>
#include <sys/event.h>
#include <unistd.h>

int scoop_terminal_watch_start(scoop_terminal *terminal) {
    terminal->notify_read = kqueue();
    if (terminal->notify_read < 0)
        return -1;
    struct kevent change;
    EV_SET(&change, SIGWINCH, EVFILT_SIGNAL, EV_ADD | EV_CLEAR, 0, 0, NULL);
    if (fcntl(terminal->notify_read, F_SETFD, FD_CLOEXEC) < 0 ||
        kevent(terminal->notify_read, &change, 1, NULL, 0, NULL) < 0) {
        int error = errno;
        (void)close(terminal->notify_read);
        terminal->notify_read = -1;
        errno = error;
        return -1;
    }
    return 0;
}

int scoop_terminal_watch_stop(scoop_terminal *terminal) {
    int result = close(terminal->notify_read);
    terminal->notify_read = -1;
    return result;
}

int scoop_terminal_watch_drain(scoop_terminal *terminal) {
    struct kevent event;
    const struct timespec immediate = {0};
    int result;
    do {
        result = kevent(terminal->notify_read, NULL, 0, &event, 1, &immediate);
    } while (result < 0 && errno == EINTR);
    if (result > 0 && (event.flags & EV_ERROR)) {
        errno = (int)event.data;
        return -1;
    }
    return result < 0 ? -1 : 0;
}
