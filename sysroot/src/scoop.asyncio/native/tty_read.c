#include "tty.h"
#include <errno.h>

void scoop_io_tty_read_ready(ScoopIoTty *tty) {
    if (tty->eof) {
        scoop_io_tty_end_read(tty, SCOOP_IO_EOF, 0, 0);
        return;
    }
    int ready = scoop_terminal_wait(tty->terminal, 0);
    if (ready < 0) {
        if (errno != EINTR)
            scoop_io_tty_end_read(tty, SCOOP_IO_POSIX_ERROR, errno, 0);
        return;
    }
    if (ready & 2) {
        scoop_terminal_size size = scoop_terminal_get_size(tty->terminal);
        if (size.columns < 0) {
            scoop_io_tty_end_read(tty, SCOOP_IO_POSIX_ERROR, errno, 0);
            return;
        }
        if (size.columns != tty->columns || size.rows != tty->rows) {
            tty->columns = size.columns;
            tty->rows = size.rows;
            int64_t packed = (size.columns << 32) | size.rows;
            scoop_io_tty_end_read(tty, SCOOP_IO_TTY_RESIZE, 0, packed);
            return;
        }
    }
    if (!(ready & 5))
        return;
    ScoopIoTtyRequest *request = tty->read;
    int64_t count = scoop_terminal_read(tty->terminal, request->bytes, request->count, ready & 4);
    if (count < 0) {
        if (errno != EINTR && errno != EAGAIN && errno != EWOULDBLOCK)
            scoop_io_tty_end_read(tty, SCOOP_IO_POSIX_ERROR, errno, 0);
    } else {
        tty->eof = count == 0;
        scoop_io_tty_end_read(tty, count == 0 ? SCOOP_IO_EOF : SCOOP_IO_BYTES, 0, count);
    }
}

static void notification_ready(uv_poll_t *handle, int status, int events) {
    (void)events;
    ScoopIoTty *tty = handle->data;
    if (!tty->read)
        return;
    if (status < 0)
        scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, status, 0);
    else
        scoop_io_tty_read_ready(tty);
}

int scoop_io_tty_receive(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count, int64_t seconds,
                         int nanos) {
    ScoopIoTtyRequest *request = calloc(1, sizeof(*request));
    if (!request)
        return UV_ENOMEM;
    request->completion.event.id = id;
    request->bytes = bytes;
    request->count = count;
    tty->read = request;
    scoop_io_tty_read_ready(tty);
    if (!tty->read)
        return 0;
    if (seconds == 0 && nanos == 0) {
        scoop_io_tty_end_read(tty, SCOOP_IO_TTY_TIMEOUT, 0, 0);
        return 0;
    }
    int status = uv_poll_start(&tty->notification, UV_READABLE, notification_ready);
    if (status == 0)
        status = scoop_io_tty_refresh(tty);
    if (status == 0 && seconds >= 0)
        status = scoop_io_tty_deadline(tty, seconds, nanos);
    if (status < 0)
        scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, status, 0);
    return 0;
}
