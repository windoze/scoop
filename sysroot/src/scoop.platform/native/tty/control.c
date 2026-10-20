#define _GNU_SOURCE 1
#include "terminal.h"
#include <errno.h>
#include <sys/ioctl.h>

int scoop_terminal_set_attributes(int fd, const struct termios *attributes) {
    int result;
    do {
        result = tcsetattr(fd, TCSANOW, attributes);
    } while (result < 0 && errno == EINTR);
    return result;
}

int scoop_terminal_set_raw(scoop_terminal *terminal, bool enabled) {
    if (terminal->raw == enabled)
        return 0;
    if (enabled) {
        struct termios original;
        if (tcgetattr(terminal->fd, &original) < 0)
            return -1;
        struct termios raw = original;
        cfmakeraw(&raw);
        terminal->saved = original;
        terminal->raw = true;
        if (scoop_terminal_set_attributes(terminal->fd, &raw) < 0) {
            int error = errno;
            if (scoop_terminal_set_attributes(terminal->fd, &original) == 0)
                terminal->raw = false;
            errno = error;
            return -1;
        }
    } else if (scoop_terminal_set_attributes(terminal->fd, &terminal->saved) < 0) {
        return -1;
    }
    terminal->raw = enabled;
    return 0;
}

bool scoop_terminal_raw_enabled(scoop_terminal *terminal) { return terminal->raw; }

scoop_terminal_size scoop_terminal_get_size(scoop_terminal *terminal) {
    struct winsize size;
    if (ioctl(terminal->fd, TIOCGWINSZ, &size) < 0)
        return (scoop_terminal_size){.columns = -1};
    return (scoop_terminal_size){size.ws_col, size.ws_row, size.ws_xpixel, size.ws_ypixel};
}
