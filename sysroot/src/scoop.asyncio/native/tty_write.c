#include "tty.h"
#include <errno.h>

void scoop_io_tty_write_ready(ScoopIoTty *tty) {
    ScoopIoTtyRequest *request = tty->write;
    int64_t count = scoop_terminal_try_write(tty->terminal, request->bytes, request->count);
    if (count >= 0)
        scoop_io_tty_end_write(tty, SCOOP_IO_BYTES, 0, count);
    else if (errno != EAGAIN && errno != EWOULDBLOCK)
        scoop_io_tty_end_write(tty, SCOOP_IO_POSIX_ERROR, errno, 0);
}

int scoop_io_tty_write(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count) {
    ScoopIoTtyRequest *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    request->bytes = bytes;
    request->count = count;
    tty->write = request;
    scoop_io_tty_write_ready(tty);
    if (tty->write) {
        int status = scoop_io_tty_refresh(tty);
        if (status < 0)
            scoop_io_tty_end_write(tty, SCOOP_IO_UV_ERROR, status, 0);
    }
    return 0;
}
