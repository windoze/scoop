#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0a00
#include <errno.h>
#include <process.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#include <windows.h>

typedef struct {
    HPCON console;
    HANDLE input, output, process, reader;
    CRITICAL_SECTION lock;
    CONDITION_VARIABLE ready;
    unsigned char *bytes;
    size_t size, capacity;
    int error;
    bool finished;
} test_pty;

static DWORD input_mode, output_mode, input_page, output_page;

int scoop_readline_test_capture_modes(void) {
    if (!GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &input_mode) ||
        !GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &output_mode)) {
        errno = EIO;
        return -1;
    }
    input_page = GetConsoleCP();
    output_page = GetConsoleOutputCP();
    return 0;
}

bool scoop_readline_test_current_modes_restored(void) {
    DWORD input, output;
    return GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &input) &&
           GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &output) && input == input_mode &&
           output == output_mode && GetConsoleCP() == input_page &&
           GetConsoleOutputCP() == output_page;
}

/* Keep draining while a child writes or ClosePseudoConsole emits its final frame. */
static unsigned __stdcall read_output(void *context) {
    test_pty *pty = context;
    unsigned char bytes[4096];
    DWORD count;
    while (ReadFile(pty->output, bytes, sizeof(bytes), &count, NULL) && count) {
        EnterCriticalSection(&pty->lock);
        if (!pty->error && count > SIZE_MAX - pty->size)
            pty->error = ENOMEM;
        if (!pty->error && pty->size + count > pty->capacity) {
            size_t capacity = pty->size + count;
            unsigned char *grown = realloc(pty->bytes, capacity);
            if (!grown)
                pty->error = ENOMEM;
            else {
                pty->bytes = grown;
                pty->capacity = capacity;
            }
        }
        if (!pty->error) {
            memcpy(pty->bytes + pty->size, bytes, count);
            pty->size += count;
        }
        WakeAllConditionVariable(&pty->ready);
        LeaveCriticalSection(&pty->lock);
    }
    EnterCriticalSection(&pty->lock);
    pty->finished = true;
    WakeAllConditionVariable(&pty->ready);
    LeaveCriticalSection(&pty->lock);
    return 0;
}

void scoop_readline_test_pty_close(test_pty *pty) {
    if (pty->process) {
        if (WaitForSingleObject(pty->process, 0) == WAIT_TIMEOUT) {
            TerminateProcess(pty->process, 125);
            WaitForSingleObject(pty->process, INFINITE);
        }
        CloseHandle(pty->process);
    }
    if (pty->input)
        CloseHandle(pty->input);
    if (pty->console)
        ClosePseudoConsole(pty->console);
    if (pty->reader) {
        WaitForSingleObject(pty->reader, INFINITE);
        CloseHandle(pty->reader);
    }
    if (pty->output)
        CloseHandle(pty->output);
    DeleteCriticalSection(&pty->lock);
    free(pty->bytes);
    free(pty);
}

static wchar_t *wide(const char *text) {
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, NULL, 0);
    if (count <= 0)
        return NULL;
    wchar_t *result = calloc((size_t)count, sizeof(wchar_t));
    if (result && !MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, result, count)) {
        free(result);
        return NULL;
    }
    return result;
}

static wchar_t *quote(wchar_t *out, const wchar_t *text) {
    *out++ = L'"';
    while (*text) {
        size_t slashes = 0;
        while (*text == L'\\') {
            ++slashes;
            ++text;
        }
        size_t copies = (!*text || *text == L'"') ? slashes * 2 : slashes;
        while (copies--)
            *out++ = L'\\';
        if (*text == L'"')
            *out++ = L'\\';
        if (*text)
            *out++ = *text++;
    }
    *out++ = L'"';
    return out;
}

static wchar_t *child_environment(void) {
    wchar_t *original = GetEnvironmentStringsW();
    if (!original)
        return NULL;
    const wchar_t term[] = L"TERM=xterm-256color";
    size_t size = sizeof(term) / sizeof(wchar_t) + 1;
    for (const wchar_t *item = original; *item; item += wcslen(item) + 1)
        size += wcslen(item) + 1;
    wchar_t *result = calloc(size, sizeof(wchar_t));
    if (result) {
        wchar_t *out = result;
        bool inserted = false;
        for (const wchar_t *item = original; *item; item += wcslen(item) + 1) {
            if (!_wcsnicmp(item, L"SCOOP_TEST_", 11) || !_wcsnicmp(item, L"TERM=", 5))
                continue;
            if (!inserted && _wcsicmp(item, term) > 0) {
                memcpy(out, term, sizeof(term));
                out += sizeof(term) / sizeof(wchar_t);
                inserted = true;
            }
            size_t count = wcslen(item) + 1;
            memcpy(out, item, count * sizeof(wchar_t));
            out += count;
        }
        if (!inserted)
            memcpy(out, term, sizeof(term));
    }
    FreeEnvironmentStringsW(original);
    return result;
}

test_pty *scoop_readline_test_pty_open(const char *executable, const char *selector) {
    test_pty *pty = calloc(1, sizeof(*pty));
    if (!pty)
        return NULL;
    InitializeCriticalSection(&pty->lock);
    InitializeConditionVariable(&pty->ready);
    HANDLE input = NULL, output = NULL;
    wchar_t *file = wide(executable), *name = wide(selector);
    wchar_t *environment = child_environment(), *command = NULL;
    STARTUPINFOEXW startup = {0};
    startup.StartupInfo.cb = sizeof(startup);
    /* Null std handles select ConPTY instead of duplicating the runner's redirections. */
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    bool attributes = false, success = false;
    if (!file || !name || !environment)
        goto done;
    const wchar_t filter[] = L" --scoop-test-filter-ignored ";
    size_t count = 2 * (wcslen(file) + wcslen(name)) + sizeof(filter) / sizeof(wchar_t) + 5;
    command = calloc(count, sizeof(wchar_t));
    if (!command)
        goto done;
    wchar_t *out = quote(command, file);
    memcpy(out, filter, sizeof(filter) - sizeof(wchar_t));
    out += sizeof(filter) / sizeof(wchar_t) - 1;
    *quote(out, name) = 0;
    if (!CreatePipe(&input, &pty->input, NULL, 0) || !CreatePipe(&pty->output, &output, NULL, 0))
        goto done;
    pty->reader = (HANDLE)_beginthreadex(NULL, 0, read_output, pty, 0, NULL);
    if (!pty->reader)
        goto done;
    COORD size = {24, 8};
    if (FAILED(CreatePseudoConsole(size, input, output, 0, &pty->console)))
        goto done;
    SIZE_T bytes = 0;
    InitializeProcThreadAttributeList(NULL, 1, 0, &bytes);
    startup.lpAttributeList = malloc(bytes);
    if (!startup.lpAttributeList ||
        !InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &bytes))
        goto done;
    attributes = true;
    if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                                   pty->console, sizeof(pty->console), NULL, NULL))
        goto done;
    PROCESS_INFORMATION process;
    if (!CreateProcessW(file, command, NULL, NULL, FALSE,
                        EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT, environment,
                        NULL, &startup.StartupInfo, &process))
        goto done;
    pty->process = process.hProcess;
    CloseHandle(process.hThread);
    success = true;
done:
    if (attributes)
        DeleteProcThreadAttributeList(startup.lpAttributeList);
    free(startup.lpAttributeList);
    free(file);
    free(name);
    free(environment);
    free(command);
    if (input)
        CloseHandle(input);
    if (output)
        CloseHandle(output);
    if (!success) {
        scoop_readline_test_pty_close(pty);
        errno = EIO;
        return NULL;
    }
    return pty;
}

int64_t scoop_readline_test_pty_read(test_pty *pty, void *bytes, int64_t count) {
    EnterCriticalSection(&pty->lock);
    if (!pty->size && !pty->error && !pty->finished)
        SleepConditionVariableCS(&pty->ready, &pty->lock, 25);
    if (pty->error) {
        errno = pty->error;
        LeaveCriticalSection(&pty->lock);
        return -1;
    }
    size_t size = (uint64_t)count < pty->size ? (size_t)count : pty->size;
    if (size) {
        memcpy(bytes, pty->bytes, size);
        pty->size -= size;
        memmove(pty->bytes, pty->bytes + size, pty->size);
    }
    LeaveCriticalSection(&pty->lock);
    return (int64_t)size;
}

int64_t scoop_readline_test_pty_write(test_pty *pty, const void *bytes, int64_t count) {
    DWORD written;
    DWORD size = (uint64_t)count > MAXDWORD ? MAXDWORD : (DWORD)count;
    if (!WriteFile(pty->input, bytes, size, &written, NULL)) {
        errno = EIO;
        return -1;
    }
    return written;
}

int scoop_readline_test_pty_wait(test_pty *pty) {
    DWORD status = WaitForSingleObject(pty->process, 5000), code;
    if (status != WAIT_OBJECT_0 || !GetExitCodeProcess(pty->process, &code)) {
        errno = status == WAIT_TIMEOUT ? ETIMEDOUT : EIO;
        return -1;
    }
    return (int)code;
}

int scoop_readline_test_pty_resize(test_pty *pty, int rows, int columns) {
    COORD size = {(SHORT)columns, (SHORT)rows};
    if (FAILED(ResizePseudoConsole(pty->console, size))) {
        errno = EIO;
        return -1;
    }
    return 0;
}
