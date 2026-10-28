#include "terminal.h"
#include <string.h>

static void encode(scoop_terminal *terminal, uint32_t scalar) {
    unsigned char *bytes = terminal->bytes + terminal->end;
    if (scalar < 0x80) {
        *bytes = (unsigned char)scalar;
        terminal->end += 1;
    } else if (scalar < 0x800) {
        bytes[0] = (unsigned char)(0xc0 | (scalar >> 6));
        bytes[1] = (unsigned char)(0x80 | (scalar & 0x3f));
        terminal->end += 2;
    } else if (scalar < 0x10000) {
        bytes[0] = (unsigned char)(0xe0 | (scalar >> 12));
        bytes[1] = (unsigned char)(0x80 | ((scalar >> 6) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | (scalar & 0x3f));
        terminal->end += 3;
    } else {
        bytes[0] = (unsigned char)(0xf0 | (scalar >> 18));
        bytes[1] = (unsigned char)(0x80 | ((scalar >> 12) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | ((scalar >> 6) & 0x3f));
        bytes[3] = (unsigned char)(0x80 | (scalar & 0x3f));
        terminal->end += 4;
    }
}

static void character(scoop_terminal *terminal, WCHAR unit) {
    if (terminal->high_surrogate != 0) {
        WCHAR high = terminal->high_surrogate;
        terminal->high_surrogate = 0;
        if (unit >= 0xdc00 && unit <= 0xdfff) {
            encode(terminal, 0x10000 + ((uint32_t)(high - 0xd800) << 10) + (unit - 0xdc00));
            return;
        }
        /* The shared strict UTF-8 parser reports an unpaired UTF-16 unit. */
        encode(terminal, high);
    }
    if (unit >= 0xd800 && unit <= 0xdbff)
        terminal->high_surrogate = unit;
    else
        encode(terminal, unit);
}

static int disconnected(scoop_terminal *terminal) {
    DWORD error = GetLastError();
    if (error != ERROR_INVALID_HANDLE && error != ERROR_BROKEN_PIPE)
        return -1;
    terminal->ended = true;
    if (terminal->high_surrogate != 0) {
        encode(terminal, terminal->high_surrogate);
        terminal->high_surrogate = 0;
        return 5;
    }
    return 4;
}

/* One bounded batch also lets a zero-time poll make progress without draining forever. */
static int pump(scoop_terminal *terminal) {
    for (unsigned consumed = 0; consumed < 64; ++consumed) {
        int flags = terminal->resized ? 2 : 0;
        if (terminal->begin != terminal->end)
            return flags | 1;
        if (terminal->ended)
            return flags | 4;
        if (flags != 0)
            return flags;
        terminal->begin = terminal->end = 0;
        if (terminal->repeats != 0) {
            --terminal->repeats;
            character(terminal, terminal->repeat_unit);
            continue;
        }
        INPUT_RECORD record;
        DWORD count;
        if (!PeekConsoleInputW(terminal->input, &record, 1, &count))
            return disconnected(terminal);
        if (count == 0)
            return 0;
        if (!ReadConsoleInputW(terminal->input, &record, 1, &count))
            return disconnected(terminal);
        if (count == 0)
            return 0;
        if (record.EventType == WINDOW_BUFFER_SIZE_EVENT) {
            terminal->resized = true;
        } else if (record.EventType == KEY_EVENT) {
            KEY_EVENT_RECORD key = record.Event.KeyEvent;
            /* ConPTY and Alt+numpad deliver Unicode text when Alt is released. */
            if (!key.bKeyDown && !(key.wVirtualKeyCode == VK_MENU && key.uChar.UnicodeChar != 0))
                continue;
            bool control_nul = (key.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) &&
                               (key.wVirtualKeyCode == VK_SPACE || key.wVirtualKeyCode == '2' ||
                                key.wVirtualKeyCode == '@');
            if (key.uChar.UnicodeChar != 0 || control_nul) {
                terminal->repeat_unit = key.uChar.UnicodeChar;
                terminal->repeats = key.wRepeatCount;
            }
        }
    }
    return (terminal->begin != terminal->end ? 1 : 0) | (terminal->resized ? 2 : 0);
}

int scoop_terminal_wait(scoop_terminal *terminal, int milliseconds) {
    ULONGLONG started = GetTickCount64();
    for (;;) {
        int ready = pump(terminal);
        if (ready != 0) {
            if (ready > 0)
                terminal->resized = false;
            return ready;
        }
        DWORD remaining = INFINITE;
        if (milliseconds >= 0) {
            ULONGLONG elapsed = GetTickCount64() - started;
            if (elapsed >= (DWORD)milliseconds)
                return 0;
            remaining = (DWORD)milliseconds - (DWORD)elapsed;
        }
        DWORD waited = WaitForSingleObject(terminal->input, remaining);
        if (waited == WAIT_TIMEOUT)
            return 0;
        if (waited == WAIT_FAILED)
            return disconnected(terminal);
    }
}

int64_t scoop_terminal_read(scoop_terminal *terminal, void *bytes, int64_t count, bool hung_up) {
    (void)hung_up;
    if (count == 0)
        return 0;
    int64_t copied = 0;
    while (copied < count) {
        int ready = pump(terminal);
        if (ready < 0)
            return copied != 0 ? copied : -1;
        if (!(ready & 1)) {
            if (copied != 0 || (ready & 4))
                return copied;
            SetLastError(ERROR_IO_INCOMPLETE);
            return -1;
        }
        unsigned length = terminal->end - terminal->begin;
        if (count - copied < length)
            length = (unsigned)(count - copied);
        memcpy((unsigned char *)bytes + copied, terminal->bytes + terminal->begin, length);
        terminal->begin += length;
        copied += length;
    }
    return copied;
}
