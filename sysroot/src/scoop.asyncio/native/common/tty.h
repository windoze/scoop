#ifndef SCOOP_ASYNCIO_COMMON_TTY_H
#define SCOOP_ASYNCIO_COMMON_TTY_H

#include "../loop.h"
#include "../../../scoop.platform/native/tty/terminal.h"
#include <errno.h>

typedef struct {
    scoop_terminal_size size;
    ScoopIoStatus status;
} ScoopIoTtySize;

static inline ScoopIoStatus scoop_io_terminal_status(bool failed) {
#ifdef _WIN32
    return (ScoopIoStatus){SCOOP_IO_WIN32_ERROR, failed ? (int32_t)GetLastError() : 0};
#else
    return scoop_io_errno(failed ? -errno : 0);
#endif
}

static inline ScoopIoStatus scoop_io_terminal_raw(scoop_terminal *terminal, bool enabled) {
    return scoop_io_terminal_status(scoop_terminal_set_raw(terminal, enabled) < 0);
}

static inline ScoopIoTtySize scoop_io_terminal_size(scoop_terminal *terminal) {
    scoop_terminal_size size = scoop_terminal_get_size(terminal);
    return (ScoopIoTtySize){size, scoop_io_terminal_status(size.columns < 0)};
}

#endif
