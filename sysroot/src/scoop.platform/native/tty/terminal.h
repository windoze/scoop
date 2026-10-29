#ifndef SCOOP_PLATFORM_TTY_H
#define SCOOP_PLATFORM_TTY_H

#include <stdbool.h>
#include <stdint.h>

#ifdef _WIN32
#include "../windows_api.h"

typedef struct {
    HANDLE input, output;
    DWORD input_mode, output_mode, saved_raw_mode;
    UINT output_code_page;
    bool raw, input_changed, output_changed, code_page_changed;
    bool resized, ended;
    WCHAR high_surrogate, repeat_unit;
    WORD repeats;
    unsigned char bytes[8];
    unsigned begin, end;
} scoop_terminal;

#else
#include <signal.h>
#include <stdatomic.h>
#include <termios.h>

typedef struct {
    int fd;
    bool raw;
    struct termios saved;
    int notify_read;
#ifdef __linux__
    int notify_write;
    bool watching;
    struct sigaction previous;
    atomic_bool previous_called;
#endif
} scoop_terminal;
#endif

typedef struct {
    int64_t columns, rows, width_pixels, height_pixels;
} scoop_terminal_size;

int scoop_terminal_set_raw(scoop_terminal *terminal, bool enabled);
#ifndef _WIN32
int scoop_terminal_set_attributes(int fd, const struct termios *attributes);
int scoop_terminal_watch_start(scoop_terminal *terminal);
int scoop_terminal_watch_stop(scoop_terminal *terminal);
int scoop_terminal_watch_drain(scoop_terminal *terminal);
#endif
bool scoop_terminal_is_tty(int stream);
void *scoop_terminal_open(void);
int scoop_terminal_close(scoop_terminal *terminal);
bool scoop_terminal_raw_enabled(scoop_terminal *terminal);
scoop_terminal_size scoop_terminal_get_size(scoop_terminal *terminal);
int scoop_terminal_wait(scoop_terminal *terminal, int milliseconds);
int64_t scoop_terminal_read(scoop_terminal *terminal, void *bytes, int64_t count, bool hung_up);
int64_t scoop_terminal_try_write(scoop_terminal *terminal, const void *bytes, int64_t count);
int64_t scoop_terminal_write(scoop_terminal *terminal, const void *bytes, int64_t count);

#endif
