#include "../../../native/windows/tty.h"
#include <errno.h>

typedef struct {
    HANDLE input, output;
    DWORD input_mode, output_mode;
    UINT output_code_page;
    CONSOLE_SCREEN_BUFFER_INFO original;
    COORD cursor;
    bool detached;
} TestTerminal;

/* Run this Cone in an isolated hidden console; no test allocates a visible window. */
ScoopIoCreated scoop_test_tty_create(void) {
    TestTerminal *terminal = calloc(1, sizeof(*terminal));
    if (!terminal)
        return (ScoopIoCreated){NULL, scoop_io_errno(-ENOMEM)};
    terminal->input = CreateFileW(L"CONIN$", GENERIC_READ | GENERIC_WRITE,
                                  FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
    terminal->output =
        CreateFileW(L"CONOUT$", GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                    NULL, OPEN_EXISTING, 0, NULL);
    if (terminal->input == INVALID_HANDLE_VALUE || terminal->output == INVALID_HANDLE_VALUE ||
        !GetConsoleMode(terminal->input, &terminal->input_mode) ||
        !GetConsoleMode(terminal->output, &terminal->output_mode) ||
        !GetConsoleScreenBufferInfo(terminal->output, &terminal->original) ||
        !FlushConsoleInputBuffer(terminal->input))
        goto failed;
    terminal->output_code_page = GetConsoleOutputCP();
    if (!terminal->output_code_page)
        goto failed;
    terminal->cursor = terminal->original.dwCursorPosition;
    return (ScoopIoCreated){terminal, {0}};
failed: {
    DWORD error = GetLastError();
    if (terminal->input != INVALID_HANDLE_VALUE)
        CloseHandle(terminal->input);
    if (terminal->output != INVALID_HANDLE_VALUE)
        CloseHandle(terminal->output);
    free(terminal);
    return (ScoopIoCreated){NULL, scoop_io_win32(error)};
}
}

int scoop_test_tty_send(TestTerminal *terminal, int value) {
    INPUT_RECORD record = {0};
    record.EventType = KEY_EVENT;
    record.Event.KeyEvent = (KEY_EVENT_RECORD){TRUE, 1, 0, 0, {(WCHAR)value}, 0};
    DWORD count;
    return WriteConsoleInputW(terminal->input, &record, 1, &count) && count == 1
               ? 0
               : (int)GetLastError();
}

int scoop_test_tty_receive(TestTerminal *terminal) {
    WCHAR text;
    DWORD count;
    if (!ReadConsoleOutputCharacterW(terminal->output, &text, 1, terminal->cursor, &count))
        return -(int)GetLastError();
    if (count != 1)
        return -ERROR_READ_FAULT;
    terminal->cursor.X++;
    return text;
}

static bool resize(HANDLE output, SMALL_RECT window, COORD buffer) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(output, &info))
        return false;
    COORD expanded = {info.dwSize.X > buffer.X ? info.dwSize.X : buffer.X,
                      info.dwSize.Y > buffer.Y ? info.dwSize.Y : buffer.Y};
    return SetConsoleScreenBufferSize(output, expanded) &&
           SetConsoleWindowInfo(output, TRUE, &window) &&
           SetConsoleScreenBufferSize(output, buffer);
}

int scoop_test_tty_resize(TestTerminal *terminal, int columns, int rows) {
    SMALL_RECT window = {0, 0, (SHORT)(columns - 1), (SHORT)(rows - 1)};
    COORD buffer = {(SHORT)columns, (SHORT)rows};
    if (!resize(terminal->output, window, buffer))
        return (int)GetLastError();
    INPUT_RECORD record = {0};
    record.EventType = WINDOW_BUFFER_SIZE_EVENT;
    record.Event.WindowBufferSizeEvent.dwSize = buffer;
    DWORD count;
    return WriteConsoleInputW(terminal->input, &record, 1, &count) && count == 1
               ? 0
               : (int)GetLastError();
}

int scoop_test_tty_end_input(TestTerminal *terminal) {
    if (!FreeConsole())
        return (int)GetLastError();
    terminal->detached = true;
    return 0;
}

bool scoop_test_tty_restored(TestTerminal *terminal) {
    DWORD input, output;
    return GetConsoleMode(terminal->input, &input) && GetConsoleMode(terminal->output, &output) &&
           input == terminal->input_mode && output == terminal->output_mode &&
           GetConsoleOutputCP() == terminal->output_code_page;
}

void scoop_test_tty_close(TestTerminal *terminal) {
    if (terminal->detached) {
        if (!AttachConsole(ATTACH_PARENT_PROCESS))
            abort();
    } else {
        if (!resize(terminal->output, terminal->original.srWindow, terminal->original.dwSize))
            abort();
        if (!SetConsoleCursorPosition(terminal->output, terminal->original.dwCursorPosition))
            abort();
        if (!CloseHandle(terminal->input) || !CloseHandle(terminal->output))
            abort();
    }
    free(terminal);
}

int scoop_test_tty_read_started(ScoopIoTty *tty) {
    ULONGLONG deadline = GetTickCount64() + 5000;
    if (!tty->input)
        return ERROR_INVALID_STATE;
    /* The first field of each request is its actual worker record. */
    ScoopIoWork *work = (void *)tty->input;
    for (;;) {
        AcquireSRWLockExclusive(&tty->loop->mutex);
        bool started = work->state != SCOOP_WORK_QUEUED;
        ReleaseSRWLockExclusive(&tty->loop->mutex);
        if (started)
            return 0;
        if (GetTickCount64() >= deadline)
            return ERROR_TIMEOUT;
        Sleep(1);
    }
}
