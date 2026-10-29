#ifndef SCOOP_ASYNCIO_WINDOWS_TTY_H
#define SCOOP_ASYNCIO_WINDOWS_TTY_H

#include "../common/tty.h"
#include "backend.h"

typedef struct ScoopIoTtyIo ScoopIoTtyIo;

typedef struct {
    ScoopIoWork closing;
    ScoopIoLoop *loop;
    scoop_terminal *terminal;
    ScoopIoTtyIo *input, *output;
    int64_t columns, rows;
    bool is_closing;
} ScoopIoTty;

void scoop_io_tty_idle(ScoopIoTty *tty);
void scoop_io_tty_cancel_read(ScoopIoTty *tty, int64_t id);
void scoop_io_tty_cancel_write(ScoopIoTty *tty, int64_t id);
int64_t scoop_io_tty_request_id(const ScoopIoTtyIo *request);

#endif
