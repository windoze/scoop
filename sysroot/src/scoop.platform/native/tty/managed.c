#define _GNU_SOURCE 1
#include "managed.h"
#include <errno.h>
#include <stddef.h>

static int32_t error_code(bool failed) {
    if (!failed)
        return 0;
#ifdef _WIN32
    return (int32_t)GetLastError();
#else
    return errno;
#endif
}

scoop_terminal_opened scoop_platform_terminal_open(void) {
    void *value = scoop_terminal_open();
    return (scoop_terminal_opened){value, error_code(value == NULL)};
}
scoop_terminal_status scoop_platform_terminal_close(scoop_terminal *terminal) {
    int value = scoop_terminal_close(terminal);
    return (scoop_terminal_status){value, error_code(value < 0)};
}
scoop_terminal_status scoop_platform_terminal_raw(scoop_terminal *terminal, bool enabled) {
    int value = scoop_terminal_set_raw(terminal, enabled);
    return (scoop_terminal_status){value, error_code(value < 0)};
}
scoop_terminal_dimensions scoop_platform_terminal_size(scoop_terminal *terminal) {
    scoop_terminal_size value = scoop_terminal_get_size(terminal);
    return (scoop_terminal_dimensions){value, error_code(value.columns < 0)};
}
scoop_terminal_count scoop_platform_terminal_write(scoop_terminal *terminal, const void *bytes,
                                                   int64_t count) {
    int64_t value = scoop_terminal_write(terminal, bytes, count);
    return (scoop_terminal_count){value, error_code(value < 0)};
}
scoop_terminal_status scoop_platform_terminal_wait(scoop_terminal *terminal, int milliseconds) {
    int value = scoop_terminal_wait(terminal, milliseconds);
    return (scoop_terminal_status){value, error_code(value < 0)};
}
scoop_terminal_count scoop_platform_terminal_read(scoop_terminal *terminal, void *bytes,
                                                  int64_t count, bool hung_up) {
    int64_t value = scoop_terminal_read(terminal, bytes, count, hung_up);
    return (scoop_terminal_count){value, error_code(value < 0)};
}
