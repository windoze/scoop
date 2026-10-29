#include "terminal.h"

int scoop_terminal_set_raw(scoop_terminal *terminal, bool enabled) {
    if (terminal->raw == enabled)
        return 0;
    if (enabled) {
        DWORD mode;
        if (!GetConsoleMode(terminal->input, &mode))
            return -1;
        terminal->saved_raw_mode = mode;
        DWORD raw =
            (mode | ENABLE_EXTENDED_FLAGS | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT) &
            ~(DWORD)(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT |
                     ENABLE_QUICK_EDIT_MODE);
        if (!SetConsoleMode(terminal->input, raw))
            return -1;
    } else if (!SetConsoleMode(terminal->input, terminal->saved_raw_mode)) {
        return -1;
    }
    terminal->raw = enabled;
    return 0;
}

bool scoop_terminal_raw_enabled(scoop_terminal *terminal) { return terminal->raw; }

scoop_terminal_size scoop_terminal_get_size(scoop_terminal *terminal) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(terminal->output, &info))
        return (scoop_terminal_size){.columns = -1};
    return (scoop_terminal_size){info.srWindow.Right - info.srWindow.Left + 1,
                                 info.srWindow.Bottom - info.srWindow.Top + 1, 0, 0};
}
