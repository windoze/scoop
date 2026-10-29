#ifndef SCOOP_PLATFORM_TTY_MANAGED_H
#define SCOOP_PLATFORM_TTY_MANAGED_H
#include "terminal.h"

typedef struct {
    void *value;
    int32_t error;
} scoop_terminal_opened;
typedef struct {
    int32_t value, error;
} scoop_terminal_status;
typedef struct {
    int64_t value;
    int32_t error;
} scoop_terminal_count;
typedef struct {
    scoop_terminal_size value;
    int32_t error;
} scoop_terminal_dimensions;

scoop_terminal_opened scoop_platform_terminal_open(void);
scoop_terminal_status scoop_platform_terminal_close(scoop_terminal *terminal);
scoop_terminal_status scoop_platform_terminal_raw(scoop_terminal *terminal, bool enabled);
scoop_terminal_dimensions scoop_platform_terminal_size(scoop_terminal *terminal);
scoop_terminal_count scoop_platform_terminal_write(scoop_terminal *terminal, const void *bytes,
                                                   int64_t count);
scoop_terminal_status scoop_platform_terminal_wait(scoop_terminal *terminal, int milliseconds);
scoop_terminal_count scoop_platform_terminal_read(scoop_terminal *terminal, void *bytes,
                                                  int64_t count, bool hung_up);
#endif
