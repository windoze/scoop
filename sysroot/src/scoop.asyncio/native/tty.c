#include "tty.h"
#include <errno.h>

static void tty_closed(ScoopIoTty *tty) {
    int status = scoop_terminal_close(tty->terminal);
    ScoopIoCompletion *completion = tty->creating ? tty->creating : &tty->completion;
    if (status < 0 && !tty->creating) {
        completion->event.kind = SCOOP_IO_POSIX_ERROR;
        completion->event.code = errno;
    }
    scoop_io_complete(tty->loop, completion, 0);
    if (tty->creating)
        free(tty);
}

static void handle_closed(uv_handle_t *handle) {
    ScoopIoTty *tty = handle->data;
    if (--tty->closing_handles == 0)
        tty_closed(tty);
}

void scoop_io_tty_begin_close(ScoopIoTty *tty) {
    tty->closing_handles = tty->handles;
    if (tty->handles >= 3)
        uv_close((uv_handle_t *)&tty->timer, handle_closed);
    if (tty->handles >= 2)
        uv_close((uv_handle_t *)&tty->notification, handle_closed);
    if (tty->handles >= 1)
        uv_close((uv_handle_t *)&tty->input_output, handle_closed);
    if (!tty->handles)
        tty_closed(tty);
}

int scoop_io_tty_close(ScoopIoTty *tty, int64_t id) {
    tty->completion.event.id = id;
    scoop_io_tty_cancel_read(tty, 0);
    scoop_io_tty_cancel_write(tty, 0);
    scoop_io_tty_begin_close(tty);
    return 0;
}

int scoop_io_tty_set_raw(ScoopIoTty *tty, bool enabled) {
    return scoop_terminal_set_raw(tty->terminal, enabled);
}

bool scoop_io_tty_is_raw(ScoopIoTty *tty) { return scoop_terminal_raw_enabled(tty->terminal); }

scoop_terminal_size scoop_io_tty_size(ScoopIoTty *tty) {
    return scoop_terminal_get_size(tty->terminal);
}
