#include "tty.h"

static void deadline_tick(uv_timer_t *handle);

static int schedule_deadline(ScoopIoTty *tty) {
    uint64_t millis =
        tty->seconds > 86400 ? 86400000 : tty->seconds * 1000 + (tty->nanos + 999999) / 1000000;
    uv_update_time(&tty->loop->uv);
    return uv_timer_start(&tty->timer, deadline_tick, millis, 0);
}

static void deadline_tick(uv_timer_t *handle) {
    ScoopIoTty *tty = handle->data;
    uint64_t now = uv_hrtime();
    uint64_t elapsed = now - tty->sampled;
    uint64_t seconds = elapsed / 1000000000;
    unsigned nanos = (unsigned)(elapsed % 1000000000);
    tty->sampled = now;
    if (seconds > tty->seconds || (seconds == tty->seconds && nanos >= tty->nanos)) {
        scoop_io_tty_end_read(tty, SCOOP_IO_TTY_TIMEOUT, 0, 0);
        return;
    }
    tty->seconds -= seconds;
    if (nanos > tty->nanos) {
        tty->seconds--;
        tty->nanos += 1000000000;
    }
    tty->nanos -= nanos;
    int status = schedule_deadline(tty);
    if (status < 0)
        scoop_io_tty_end_read(tty, SCOOP_IO_UV_ERROR, status, 0);
}

int scoop_io_tty_deadline(ScoopIoTty *tty, int64_t seconds, int nanos) {
    tty->seconds = (uint64_t)seconds;
    tty->nanos = (unsigned)nanos;
    tty->sampled = uv_hrtime();
    return schedule_deadline(tty);
}
