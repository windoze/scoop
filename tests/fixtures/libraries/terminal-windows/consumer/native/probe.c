#define WIN32_LEAN_AND_MEAN
#ifdef _MSC_VER
#include <Windows.h>
#else
#include <windows.h>
#endif
#include <stdbool.h>
#include <stdint.h>

static HANDLE input, output;
static DWORD input_mode, output_mode;
static UINT input_code_page, output_code_page;

bool windows_terminal_setup(void) {
    input = CreateFileW(L"CONIN$", GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
                        NULL, OPEN_EXISTING, 0, NULL);
    output = CreateFileW(L"CONOUT$", GENERIC_READ | GENERIC_WRITE,
                         FILE_SHARE_READ | FILE_SHARE_WRITE, NULL, OPEN_EXISTING, 0, NULL);
    input_code_page = GetConsoleCP();
    output_code_page = GetConsoleOutputCP();
    return input != INVALID_HANDLE_VALUE && output != INVALID_HANDLE_VALUE &&
           GetConsoleMode(input, &input_mode) && GetConsoleMode(output, &output_mode) &&
           FlushConsoleInputBuffer(input);
}

bool windows_terminal_modes(bool raw) {
    DWORD mode, out;
    if (!GetConsoleMode(input, &mode) || !GetConsoleMode(output, &out) ||
        GetConsoleCP() != input_code_page || GetConsoleOutputCP() != CP_UTF8)
        return false;
    DWORD expected = input_mode | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_WINDOW_INPUT;
    if (raw)
        expected = (expected | ENABLE_EXTENDED_FLAGS) &
                   ~(DWORD)(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT |
                            ENABLE_QUICK_EDIT_MODE);
    return mode == expected &&
           out == (output_mode | ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
}

bool windows_terminal_restored(void) {
    DWORD mode, out;
    bool restored = GetConsoleMode(input, &mode) && GetConsoleMode(output, &out) &&
                    mode == input_mode && out == output_mode && GetConsoleCP() == input_code_page &&
                    GetConsoleOutputCP() == output_code_page;
    bool closed_input = CloseHandle(input) != 0;
    bool closed_output = CloseHandle(output) != 0;
    return restored && closed_input && closed_output;
}

bool windows_terminal_feed(int32_t unit, int32_t key, int32_t modifiers, int32_t repeats) {
    INPUT_RECORD record = {0};
    record.EventType = KEY_EVENT;
    record.Event.KeyEvent =
        (KEY_EVENT_RECORD){TRUE, (WORD)repeats, (WORD)key, 0, {(WCHAR)unit}, (DWORD)modifiers};
    DWORD mode, written;
    if (!GetConsoleMode(input, &mode))
        return false;
    /* Queue a real repeated record before VT conversion for the repetition case. */
    if (repeats > 1 && !SetConsoleMode(input, mode & ~(DWORD)ENABLE_VIRTUAL_TERMINAL_INPUT))
        return false;
    bool result = WriteConsoleInputW(input, &record, 1, &written) && written == 1;
    if (repeats > 1 && !SetConsoleMode(input, mode))
        result = false;
    return result;
}

int64_t windows_terminal_resize(void) {
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(output, &info))
        return -1;
    SMALL_RECT window = info.srWindow;
    --window.Right;
    if (!SetConsoleWindowInfo(output, TRUE, &window))
        return -1;
    INPUT_RECORD record = {0};
    record.EventType = WINDOW_BUFFER_SIZE_EVENT;
    record.Event.WindowBufferSizeEvent.dwSize = info.dwSize;
    DWORD written;
    if (!WriteConsoleInputW(input, &record, 1, &written))
        return -1;
    return window.Right - window.Left + 1;
}

bool windows_terminal_position(void) {
    return SetConsoleCursorPosition(output, (COORD){0, 0}) != 0;
}

bool windows_terminal_output(void) {
    WCHAR text[8];
    DWORD count;
    if (!ReadConsoleOutputCharacterW(output, text, 8, (COORD){0, 0}, &count) || text[0] != 0x4e2d)
        return false;
    /* This legacy cell API cannot represent supplementary Unicode scalars. */
    bool last = false;
    for (DWORD index = 0; index < count; ++index) {
        last |= text[index] == L'Z';
        if (text[index] == 0xfffd)
            return false;
    }
    return last;
}

bool windows_terminal_disconnect(void) { return FreeConsole() != 0; }
