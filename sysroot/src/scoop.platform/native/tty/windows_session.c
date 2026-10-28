#include "terminal.h"
#include <stdlib.h>

static LONG session_open;

bool scoop_terminal_is_tty(int stream) {
    static const DWORD streams[] = {STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
    DWORD mode;
    return stream >= 0 && stream < 3 && GetConsoleMode(GetStdHandle(streams[stream]), &mode);
}

static DWORD release(scoop_terminal *terminal) {
    DWORD error = 0;
    if (terminal->input_changed && !SetConsoleMode(terminal->input, terminal->input_mode))
        error = GetLastError();
    if (terminal->output_changed && !SetConsoleMode(terminal->output, terminal->output_mode) &&
        error == 0)
        error = GetLastError();
    if (terminal->code_page_changed && !SetConsoleOutputCP(terminal->output_code_page) &&
        error == 0)
        error = GetLastError();
    if (terminal->input != INVALID_HANDLE_VALUE && !CloseHandle(terminal->input) && error == 0)
        error = GetLastError();
    if (terminal->output != INVALID_HANDLE_VALUE && !CloseHandle(terminal->output) && error == 0)
        error = GetLastError();
    free(terminal);
    InterlockedExchange(&session_open, 0);
    return error;
}

void *scoop_terminal_open(void) {
    if (InterlockedCompareExchange(&session_open, 1, 0) != 0) {
        SetLastError(ERROR_BUSY);
        return NULL;
    }
    scoop_terminal *terminal = calloc(1, sizeof(*terminal));
    if (terminal == NULL) {
        InterlockedExchange(&session_open, 0);
        SetLastError(ERROR_NOT_ENOUGH_MEMORY);
        return NULL;
    }
    terminal->input = INVALID_HANDLE_VALUE;
    terminal->output = INVALID_HANDLE_VALUE;
    terminal->input = CreateFileW(L"CONIN$", GENERIC_READ | GENERIC_WRITE,
                                  FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
    if (terminal->input == INVALID_HANDLE_VALUE)
        goto failed;
    terminal->output =
        CreateFileW(L"CONOUT$", GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                    NULL, OPEN_EXISTING, 0, NULL);
    if (terminal->output == INVALID_HANDLE_VALUE ||
        !GetConsoleMode(terminal->input, &terminal->input_mode) ||
        !GetConsoleMode(terminal->output, &terminal->output_mode))
        goto failed;
    terminal->output_code_page = GetConsoleOutputCP();
    if (terminal->output_code_page == 0)
        goto failed;
    if (!SetConsoleMode(terminal->input,
                        terminal->input_mode | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT))
        goto failed;
    terminal->input_changed = true;
    if (!SetConsoleMode(terminal->output, terminal->output_mode | ENABLE_PROCESSED_OUTPUT |
                                              ENABLE_VIRTUAL_TERMINAL_PROCESSING))
        goto failed;
    terminal->output_changed = true;
    if (!SetConsoleOutputCP(CP_UTF8))
        goto failed;
    terminal->code_page_changed = true;
    return terminal;
failed: {
    DWORD error = GetLastError();
    (void)release(terminal);
    SetLastError(error);
    return NULL;
}
}

int scoop_terminal_close(scoop_terminal *terminal) {
    DWORD error = release(terminal);
    SetLastError(error);
    return error == 0 ? 0 : -1;
}

int64_t scoop_terminal_try_write(scoop_terminal *terminal, const void *bytes, int64_t count) {
    if (count == 0)
        return 0;
    DWORD written;
    DWORD length = count > INT32_MAX ? INT32_MAX : (DWORD)count;
    if (!WriteFile(terminal->output, bytes, length, &written, NULL))
        return -1;
    return written;
}

int64_t scoop_terminal_write(scoop_terminal *terminal, const void *bytes, int64_t count) {
    return scoop_terminal_try_write(terminal, bytes, count);
}
