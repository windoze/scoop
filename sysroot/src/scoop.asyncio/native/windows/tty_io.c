#include "tty.h"
#include "deadline.h"
#include "../common/platform.h"
#include <errno.h>

struct ScoopIoTtyIo {
    ScoopIoWork work;
    ScoopIoTty *tty;
    void *bytes;
    int64_t count;
    HANDLE cancel;
    ScoopIoDeadline deadline;
    bool writing, timed;
};

int64_t scoop_io_tty_request_id(const ScoopIoTtyIo *request) {
    return request->work.completion.event.id;
}

static bool receive_ready(ScoopIoTtyIo *request) {
    ScoopIoTty *tty = request->tty;
    ScoopIoEvent *event = &request->work.completion.event;
    int ready = scoop_terminal_wait(tty->terminal, 0);
    if (ready < 0) {
        scoop_io_platform_error(event, (int32_t)GetLastError());
        return true;
    }
    if (ready & 2) {
        scoop_terminal_size size = scoop_terminal_get_size(tty->terminal);
        if (size.columns < 0) {
            scoop_io_platform_error(event, (int32_t)GetLastError());
            return true;
        }
        if (size.columns != tty->columns || size.rows != tty->rows) {
            tty->columns = size.columns;
            tty->rows = size.rows;
            event->kind = SCOOP_IO_TTY_RESIZE;
            event->count = (size.columns << 32) | size.rows;
            return true;
        }
    }
    if (!(ready & 5))
        return false;
    int64_t count = scoop_terminal_read(tty->terminal, request->bytes, request->count, ready & 4);
    if (count < 0) {
        DWORD error = GetLastError();
        if (error == ERROR_IO_INCOMPLETE)
            return false;
        scoop_io_platform_error(event, (int32_t)error);
    } else {
        event->kind = count ? SCOOP_IO_BYTES : SCOOP_IO_EOF;
        event->count = count;
    }
    return true;
}

static void receive_execute(ScoopIoWork *work) {
    ScoopIoTtyIo *request = (void *)work;
    HANDLE handles[] = {request->cancel, request->tty->terminal->input};
    for (;;) {
        if (WaitForSingleObject(request->cancel, 0) == WAIT_OBJECT_0) {
            scoop_io_platform_error(&work->completion.event, ERROR_OPERATION_ABORTED);
            return;
        }
        if (receive_ready(request))
            return;
        DWORD timeout = INFINITE;
        if (request->timed) {
            LARGE_INTEGER now;
            if (!QueryPerformanceCounter(&now)) {
                scoop_io_platform_error(&work->completion.event, (int32_t)GetLastError());
                return;
            }
            timeout = scoop_io_deadline_wait(request->deadline, work->loop->frequency,
                                             (uint64_t)now.QuadPart);
            if (!timeout) {
                work->completion.event.kind = SCOOP_IO_TTY_TIMEOUT;
                return;
            }
        }
        DWORD waited = WaitForMultipleObjects(2, handles, FALSE, timeout);
        if (waited == WAIT_FAILED) {
            /* A disconnected console becomes repeatable EOF through the shared input owner. */
            DWORD error = GetLastError();
            if (!receive_ready(request))
                scoop_io_platform_error(&work->completion.event, (int32_t)error);
            return;
        }
    }
}

static void write_execute(ScoopIoWork *work) {
    ScoopIoTtyIo *request = (void *)work;
    int64_t count = scoop_terminal_write(request->tty->terminal, request->bytes, request->count);
    if (count < 0)
        scoop_io_platform_error(&work->completion.event, (int32_t)GetLastError());
    else {
        work->completion.event.kind = SCOOP_IO_BYTES;
        work->completion.event.count = count;
    }
}

static void transfer_finish(ScoopIoWork *work, int status) {
    ScoopIoTtyIo *request = (void *)work;
    ScoopIoTty *tty = request->tty;
    if (request->cancel)
        CloseHandle(request->cancel);
    if (request->writing)
        tty->output = NULL;
    else
        tty->input = NULL;
    scoop_io_work_finished(work, status);
    scoop_io_tty_idle(tty);
}

static int submit_transfer(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count, bool writing,
                           int64_t seconds, int nanos) {
    ScoopIoTtyIo *request = calloc(1, sizeof(*request));
    if (!request)
        return -ENOMEM;
    request->tty = tty;
    request->bytes = bytes;
    request->count = count;
    request->writing = writing;
    request->timed = seconds >= 0;
    DWORD error = 0;
    if (!writing) {
        request->cancel = CreateEventW(NULL, TRUE, FALSE, NULL);
        if (!request->cancel)
            error = GetLastError();
    }
    if (!error && request->timed) {
        LARGE_INTEGER now;
        if (!QueryPerformanceCounter(&now))
            error = GetLastError();
        else
            request->deadline = scoop_io_deadline_after(tty->loop->frequency,
                                                        (uint64_t)now.QuadPart, seconds, nanos);
    }
    if (error) {
        if (request->cancel)
            CloseHandle(request->cancel);
        request->work.completion.event.id = id;
        scoop_io_platform_error(&request->work.completion.event, (int32_t)error);
        scoop_io_complete(tty->loop, &request->work.completion, 0);
        return 0;
    }
    int status = scoop_io_work_submit(&request->work, tty->loop, id,
                                      writing ? write_execute : receive_execute, transfer_finish);
    if (status < 0) {
        if (request->cancel)
            CloseHandle(request->cancel);
        free(request);
    } else if (writing)
        tty->output = request;
    else
        tty->input = request;
    return status;
}

int scoop_io_tty_receive(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count, int64_t seconds,
                         int nanos) {
    return submit_transfer(tty, id, bytes, count, false, seconds, nanos);
}
int scoop_io_tty_write(ScoopIoTty *tty, int64_t id, const void *bytes, int64_t count) {
    return submit_transfer(tty, id, (void *)bytes, count, true, -1, 0);
}
void scoop_io_tty_cancel_read(ScoopIoTty *tty, int64_t id) {
    if (!tty->input || scoop_io_tty_request_id(tty->input) != id)
        return;
    if (!SetEvent(tty->input->cancel)) {
        DWORD error = GetLastError();
        AcquireSRWLockExclusive(&tty->loop->mutex);
        scoop_io_fail_locked(tty->loop, scoop_io_win32(error));
        ReleaseSRWLockExclusive(&tty->loop->mutex);
    }
    scoop_io_work_cancel(tty->loop, id);
}
void scoop_io_tty_cancel_write(ScoopIoTty *tty, int64_t id) {
    if (tty->output && scoop_io_tty_request_id(tty->output) == id)
        scoop_io_work_cancel(tty->loop, id);
}
