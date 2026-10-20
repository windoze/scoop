#ifndef SCOOP_TERMINAL_UNIX_H
#define SCOOP_TERMINAL_UNIX_H

#include <stdbool.h>
#include <stdint.h>
#include <termios.h>

typedef struct {
    int fd;
    bool raw;
    struct termios saved;
} scoop_terminal;

typedef struct {
    int64_t columns, rows, width_pixels, height_pixels;
} scoop_terminal_size;

int scoop_terminal_set_raw(scoop_terminal *terminal, bool enabled);
int scoop_terminal_set_attributes(int fd, const struct termios *attributes);

#endif
