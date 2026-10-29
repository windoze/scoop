#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#define WIN32_LEAN_AND_MEAN
#ifdef _MSC_VER
#include <Windows.h>
#else
#include <windows.h>
#endif
#include <process.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct Capture {
    HANDLE output, input_ready, resize_ready;
    char bytes[16384];
    size_t length;
    DWORD error;
};

static unsigned __stdcall capture(void *context) {
    struct Capture *data = context;
    for (;;) {
        DWORD count;
        if (!ReadFile(data->output, data->bytes + data->length,
                      (DWORD)(sizeof(data->bytes) - data->length - 1), &count, NULL)) {
            DWORD error = GetLastError();
            data->error = error == ERROR_BROKEN_PIPE ? 0 : error;
            return 0;
        }
        if (count == 0)
            return 0;
        data->length += count;
        data->bytes[data->length] = 0;
        if (strstr(data->bytes, "READY:INPUT") != NULL)
            SetEvent(data->input_ready);
        if (strstr(data->bytes, "READY:RESIZE") != NULL)
            SetEvent(data->resize_ready);
        if (data->length == sizeof(data->bytes) - 1) {
            data->error = ERROR_BUFFER_OVERFLOW;
            return 0;
        }
    }
}

static bool ready(HANDLE event, HANDLE process, HANDLE reader) {
    HANDLE handles[] = {event, process, reader};
    return WaitForMultipleObjects(3, handles, FALSE, 10000) == WAIT_OBJECT_0;
}

static void close_handle(HANDLE handle) {
    if (handle != NULL)
        CloseHandle(handle);
}

bool windows_terminal_conpty(void) {
    struct Capture data = {0};
    HANDLE input_read = NULL, input_write = NULL, output_write = NULL, reader = NULL;
    HPCON console = NULL;
    PROCESS_INFORMATION child = {0};
    STARTUPINFOEXW startup = {0};
    bool attributes = false, success = false;
    const char *stage = "pipes";
    if (!CreatePipe(&input_read, &input_write, NULL, 0) ||
        !CreatePipe(&data.output, &output_write, NULL, 0))
        goto done;
    stage = "pseudoconsole";
    if (FAILED(CreatePseudoConsole((COORD){80, 24}, input_read, output_write, 0, &console)))
        goto done;
    data.input_ready = CreateEventW(NULL, TRUE, FALSE, NULL);
    data.resize_ready = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (data.input_ready == NULL || data.resize_ready == NULL)
        goto done;
    reader = (HANDLE)_beginthreadex(NULL, 0, capture, &data, 0, NULL);
    if (reader == NULL)
        goto done;
    stage = "startup attributes";
    SIZE_T size = 0;
    InitializeProcThreadAttributeList(NULL, 1, 0, &size);
    startup.lpAttributeList = malloc(size);
    if (startup.lpAttributeList == NULL ||
        !InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &size))
        goto done;
    attributes = true;
    if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                                   console, sizeof(console), NULL, NULL))
        goto done;
    WCHAR executable[32768];
    DWORD length = GetModuleFileNameW(NULL, executable, 32768);
    if (length == 0 || length >= 32768)
        goto done;
    WCHAR command[] = L"scoop-console-child pty";
    startup.StartupInfo.cb = sizeof(startup);
    stage = "child creation";
    if (!CreateProcessW(executable, command, NULL, NULL, FALSE, EXTENDED_STARTUPINFO_PRESENT, NULL,
                        NULL, &startup.StartupInfo, &child))
        goto done;
    close_handle(input_read);
    input_read = NULL;
    close_handle(output_write);
    output_write = NULL;
    stage = "input handshake";
    if (!ready(data.input_ready, child.hProcess, reader))
        goto done;
    const char keys[] = "\xe4\xb8\xad\xf0\x9f\x99\x82\x1b[1;5D";
    DWORD written;
    if (!WriteFile(input_write, keys, sizeof(keys) - 1, &written, NULL) ||
        written != sizeof(keys) - 1)
        goto done;
    stage = "resize handshake";
    if (!ready(data.resize_ready, child.hProcess, reader) ||
        FAILED(ResizePseudoConsole(console, (COORD){81, 25})))
        goto done;
    stage = "child completion";
    DWORD status;
    if (WaitForSingleObject(child.hProcess, 10000) != WAIT_OBJECT_0 ||
        !GetExitCodeProcess(child.hProcess, &status) || status != 0)
        goto done;
    success = true;
done:
    if (child.hProcess != NULL && WaitForSingleObject(child.hProcess, 0) != WAIT_OBJECT_0) {
        TerminateProcess(child.hProcess, 1);
        WaitForSingleObject(child.hProcess, INFINITE);
    }
    close_handle(input_read);
    close_handle(output_write);
    if (console != NULL)
        ClosePseudoConsole(console);
    close_handle(input_write);
    if (reader != NULL) {
        WaitForSingleObject(reader, INFINITE);
        CloseHandle(reader);
    }
    success = success && data.error == 0 && strstr(data.bytes, "pty: passed") != NULL &&
              strstr(data.bytes, "OUT:\xe4\xb8\xad\xf0\x9f\x99\x82") != NULL;
    if (!success)
        fprintf(stderr, "ConPTY %s failed (read=%lu): %s\n", stage, data.error, data.bytes);
    if (attributes)
        DeleteProcThreadAttributeList(startup.lpAttributeList);
    free(startup.lpAttributeList);
    close_handle(child.hThread);
    close_handle(child.hProcess);
    close_handle(data.output);
    close_handle(data.input_ready);
    close_handle(data.resize_ready);
    return success;
}
