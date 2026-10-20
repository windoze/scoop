#include "tty.h"
#include <errno.h>

static int fail_open(ScoopIoTty *tty, int kind, int code) {
    tty->creating->event.kind = kind;
    tty->creating->event.code = code;
    scoop_io_tty_begin_close(tty);
    return 0;
}

int scoop_io_tty_open(ScoopIoLoop *loop, int64_t id) {
    ScoopIoTty *tty = calloc(1, sizeof(*tty));
    ScoopIoCompletion *request = calloc(1, sizeof(*request));
    if (!tty || !request) {
        free(tty);
        free(request);
        return UV_ENOMEM;
    }
    request->event.id = id;
    tty->creating = request;
    tty->loop = loop;
    tty->terminal = scoop_terminal_open();
    if (!tty->terminal) {
        request->event.kind = SCOOP_IO_POSIX_ERROR;
        request->event.code = errno;
        scoop_io_complete(loop, request, 0);
        free(tty);
        return 0;
    }
    scoop_terminal_size size = scoop_terminal_get_size(tty->terminal);
    if (size.columns < 0)
        return fail_open(tty, SCOOP_IO_POSIX_ERROR, errno);
    tty->columns = size.columns;
    tty->rows = size.rows;
    int status = uv_poll_init(&loop->uv, &tty->input_output, tty->terminal->fd);
    if (status < 0)
        return fail_open(tty, SCOOP_IO_UV_ERROR, status);
    tty->input_output.data = tty;
    tty->handles++;
    status = uv_poll_init(&loop->uv, &tty->notification, tty->terminal->notify_read);
    if (status < 0)
        return fail_open(tty, SCOOP_IO_UV_ERROR, status);
    tty->notification.data = tty;
    tty->handles++;
    status = uv_timer_init(&loop->uv, &tty->timer);
    if (status < 0)
        return fail_open(tty, SCOOP_IO_UV_ERROR, status);
    tty->timer.data = tty;
    tty->handles++;
    tty->creating = NULL;
    request->event.kind = SCOOP_IO_RESOURCE;
    request->event.resource = tty;
    scoop_io_complete(loop, request, 0);
    return 0;
}
