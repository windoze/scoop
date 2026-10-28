#include "tty.h"
#include "../common/platform.h"
#include <errno.h>

typedef struct {
    ScoopIoWork work;
    ScoopIoTty *tty;
} TtyOpen;

static void open_execute(ScoopIoWork *work) {
    TtyOpen *request = (void *)work;
    ScoopIoTty *tty = request->tty;
    tty->terminal = scoop_terminal_open();
    DWORD error = tty->terminal ? 0 : GetLastError();
    if (!error) {
        scoop_terminal_size size = scoop_terminal_get_size(tty->terminal);
        if (size.columns < 0)
            error = GetLastError();
        else {
            tty->columns = size.columns;
            tty->rows = size.rows;
        }
    }
    if (error) {
        if (tty->terminal)
            scoop_terminal_close(tty->terminal);
        tty->terminal = NULL;
        scoop_io_platform_error(&work->completion.event, (int32_t)error);
    } else {
        work->completion.event.kind = SCOOP_IO_RESOURCE;
        work->completion.event.resource = tty;
    }
}

static void open_finish(ScoopIoWork *work, int status) {
    TtyOpen *request = (void *)work;
    if (status < 0 || work->completion.event.kind != SCOOP_IO_RESOURCE)
        free(request->tty);
    scoop_io_work_finished(work, status);
}

int scoop_io_tty_open(ScoopIoLoop *loop, int64_t id) {
    TtyOpen *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->tty = calloc(1, sizeof(*request->tty));
    if (!request->tty) {
        free(request);
        return -ENOMEM;
    }
    request->tty->loop = loop;
    int status = scoop_io_work_submit(&request->work, loop, id, open_execute, open_finish);
    if (status < 0) {
        free(request->tty);
        free(request);
    }
    return status;
}

static void close_execute(ScoopIoWork *work) {
    ScoopIoTty *tty = (void *)work;
    if (scoop_terminal_close(tty->terminal) < 0)
        scoop_io_platform_error(&work->completion.event, (int32_t)GetLastError());
}

void scoop_io_tty_idle(ScoopIoTty *tty) {
    if (!tty->is_closing || tty->input || tty->output)
        return;
    int status = scoop_io_work_submit(&tty->closing, tty->loop, tty->closing.completion.event.id,
                                      close_execute, scoop_io_work_finished);
    if (status < 0) {
        /* Even a worker startup failure must consume the terminal and restore its modes. */
        close_execute(&tty->closing);
        scoop_io_complete(tty->loop, &tty->closing.completion, status);
    }
}

int scoop_io_tty_close(ScoopIoTty *tty, int64_t id) {
    tty->is_closing = true;
    tty->closing.completion.event.id = id;
    if (tty->input)
        scoop_io_tty_cancel_read(tty, scoop_io_tty_request_id(tty->input));
    if (tty->output)
        scoop_io_tty_cancel_write(tty, scoop_io_tty_request_id(tty->output));
    scoop_io_tty_idle(tty);
    return 0;
}

ScoopIoStatus scoop_io_tty_set_raw(ScoopIoTty *tty, bool enabled) {
    return scoop_io_terminal_raw(tty->terminal, enabled);
}
bool scoop_io_tty_is_raw(ScoopIoTty *tty) { return scoop_terminal_raw_enabled(tty->terminal); }
ScoopIoTtySize scoop_io_tty_size(ScoopIoTty *tty) { return scoop_io_terminal_size(tty->terminal); }
