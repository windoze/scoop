#ifndef SCOOP_ASYNCIO_TTY_H
#define SCOOP_ASYNCIO_TTY_H

#include "../../scoop.platform/native/tty/terminal.h"
#include "loop.h"

typedef struct ScoopIoTtyRequest {
    ScoopIoCompletion completion;
    void *bytes;
    int64_t count;
} ScoopIoTtyRequest;

typedef struct ScoopIoTty {
    ScoopIoCompletion completion;
    ScoopIoLoop *loop;
    scoop_terminal *terminal;
    uv_poll_t input_output;
    uv_poll_t notification;
    uv_timer_t timer;
    unsigned handles, closing_handles;
    ScoopIoCompletion *creating;
    ScoopIoTtyRequest *read, *write;
    bool eof;
    int64_t columns, rows;
    uint64_t seconds, sampled;
    unsigned nanos;
} ScoopIoTty;

int scoop_io_tty_open(ScoopIoLoop *loop, int64_t id);
int scoop_io_tty_close(ScoopIoTty *tty, int64_t id);
int scoop_io_tty_receive(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count, int64_t seconds,
                         int nanos);
int scoop_io_tty_write(ScoopIoTty *tty, int64_t id, void *bytes, int64_t count);
void scoop_io_tty_cancel_read(ScoopIoTty *tty, int64_t id);
void scoop_io_tty_cancel_write(ScoopIoTty *tty, int64_t id);
int scoop_io_tty_refresh(ScoopIoTty *tty);
void scoop_io_tty_read_ready(ScoopIoTty *tty);
void scoop_io_tty_write_ready(ScoopIoTty *tty);
void scoop_io_tty_end_read(ScoopIoTty *tty, int kind, int code, int64_t count);
void scoop_io_tty_end_write(ScoopIoTty *tty, int kind, int code, int64_t count);
void scoop_io_tty_begin_close(ScoopIoTty *tty);
int scoop_io_tty_deadline(ScoopIoTty *tty, int64_t seconds, int nanos);

#endif
