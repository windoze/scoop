#include "tty.h"

static void input_output_ready(uv_poll_t *handle, int status, int events) {
    ScoopIoTty *tty = handle->data;
    if (tty->read && (status < 0 || (events & (UV_READABLE | UV_DISCONNECT)))) {
        scoop_io_tty_read_ready(tty);
        if (status < 0 && tty->read)
            scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, status, 0);
    }
    if (tty->write && (status < 0 || (events & UV_WRITABLE))) {
        scoop_io_tty_write_ready(tty);
        if (status < 0 && tty->write)
            scoop_io_tty_end_write(tty, SCOOP_IO_UV_ERROR, status, 0);
    }
}

int scoop_io_tty_refresh(ScoopIoTty *tty) {
    int events = (tty->read ? UV_READABLE | UV_DISCONNECT : 0) | (tty->write ? UV_WRITABLE : 0);
    return events ? uv_poll_start(&tty->input_output, events, input_output_ready)
                  : uv_poll_stop(&tty->input_output);
}

void scoop_io_tty_end_read(ScoopIoTty *tty, int kind, int code, int64_t count) {
    ScoopIoTtyRequest *request = tty->read;
    tty->read = NULL;
    uv_timer_stop(&tty->timer);
    uv_poll_stop(&tty->notification);
    request->completion.event.kind = kind;
    request->completion.event.code = code;
    request->completion.event.count = count;
    scoop_io_complete(tty->loop, &request->completion, 0);
    int status = scoop_io_tty_refresh(tty);
    if (status < 0 && tty->write)
        scoop_io_tty_end_write(tty, SCOOP_IO_UV_ERROR, status, 0);
}

void scoop_io_tty_end_write(ScoopIoTty *tty, int kind, int code, int64_t count) {
    ScoopIoTtyRequest *request = tty->write;
    tty->write = NULL;
    request->completion.event.kind = kind;
    request->completion.event.code = code;
    request->completion.event.count = count;
    scoop_io_complete(tty->loop, &request->completion, 0);
    int status = scoop_io_tty_refresh(tty);
    if (status < 0 && tty->read)
        scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, status, 0);
}

void scoop_io_tty_cancel_read(ScoopIoTty *tty, int64_t id) {
    if (tty->read && (id == 0 || tty->read->completion.event.id == id))
        scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, UV_ECANCELED, 0);
}

void scoop_io_tty_cancel_write(ScoopIoTty *tty, int64_t id) {
    if (tty->write && (id == 0 || tty->write->completion.event.id == id))
        scoop_io_tty_end_write(tty, SCOOP_IO_UV_ERROR, UV_ECANCELED, 0);
}
