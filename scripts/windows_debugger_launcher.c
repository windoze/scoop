/* Relocatable LLDB entry point. The child owns debugging and the Python VM. */
#define WIN32_LEAN_AND_MEAN
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <windows.h>

static wchar_t *joined(const wchar_t *left, const wchar_t *right) {
    size_t length = wcslen(left) + wcslen(right) + 1;
    wchar_t *value = malloc(length * sizeof *value);
    if (value == NULL) {
        fputs("scoop: cannot allocate debugger launch arguments\n", stderr);
        exit(1);
    }
    wcscpy(value, left);
    wcscat(value, right);
    return value;
}

static void environment_path(const wchar_t *name, const wchar_t *root, const wchar_t *suffix) {
    wchar_t *value = joined(root, suffix);
    if (!SetEnvironmentVariableW(name, value)) {
        fputs("scoop: cannot set debugger environment\n", stderr);
        exit(1);
    }
    free(value);
}

static wchar_t *executable_path(void) {
    DWORD capacity = MAX_PATH;
    for (;;) {
        wchar_t *path = calloc(capacity, sizeof *path);
        if (path == NULL)
            return NULL;
        DWORD length = GetModuleFileNameW(NULL, path, capacity);
        if (length > 0 && length < capacity)
            return path;
        free(path);
        if (length == 0 || capacity > 32768)
            return NULL;
        capacity *= 2;
    }
}

/* Quote one argv value using the Windows CRT backslash/quote convention. */
static wchar_t *argument(wchar_t *cursor, const wchar_t *value) {
    *cursor++ = L'"';
    while (*value != 0) {
        size_t slashes = 0;
        while (*value == L'\\') {
            slashes++;
            value++;
        }
        size_t copies = (*value == L'"' || *value == 0) ? slashes * 2 : slashes;
        while (copies-- > 0)
            *cursor++ = L'\\';
        if (*value == 0)
            break;
        if (*value == L'"')
            *cursor++ = L'\\';
        *cursor++ = *value++;
    }
    *cursor++ = L'"';
    *cursor++ = L' ';
    return cursor;
}

static BOOL WINAPI console_control(DWORD event) {
    /* The child receives console events too and handles debugger interrupts. */
    return event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT;
}

int wmain(int argc, wchar_t **argv) {
    wchar_t *root = executable_path();
    if (root == NULL)
        return 1;
    wchar_t *name = wcsrchr(root, L'\\');
    if (name == NULL)
        return 1;
    int dap = _wcsicmp(name + 1, L"lldb-dap.exe") == 0;
    *name = 0;
    wchar_t *bin = wcsrchr(root, L'\\');
    if (bin == NULL)
        return 1;
    *bin = 0;
    wchar_t *child = joined(root, dap ? L"\\libexec\\lldb\\bin\\lldb-dap.exe"
                                      : L"\\libexec\\lldb\\bin\\lldb.exe");
    environment_path(L"PYTHONHOME", root, L"\\libexec\\lldb\\python");
    wchar_t *packages = joined(root, L"\\libexec\\lldb\\python\\Lib\\site-packages;");
    wchar_t *search = joined(packages, root);
    environment_path(L"PYTHONPATH", search, L"\\share\\scoop\\debugger");
    free(packages);
    free(search);
    SetEnvironmentVariableW(L"PYTHONNOUSERSITE", L"1");
    environment_path(L"LLDB_DEBUGSERVER_PATH", root, L"\\libexec\\lldb\\bin\\lldb-server.exe");
    DWORD path_length = GetEnvironmentVariableW(L"PATH", NULL, 0);
    wchar_t *old_path = calloc((size_t)path_length + 1, sizeof *old_path);
    if (old_path == NULL)
        return 1;
    if (path_length != 0)
        GetEnvironmentVariableW(L"PATH", old_path, path_length);
    wchar_t *python = joined(root, L"\\libexec\\lldb\\python;");
    wchar_t *path = joined(python, old_path);
    SetEnvironmentVariableW(L"PATH", path);
    free(python);
    free(old_path);
    free(path);

    size_t capacity = 2 * wcslen(child) + 128;
    for (int index = 1; index < argc; index++)
        capacity += 2 * wcslen(argv[index]) + 3;
    wchar_t *command = malloc(capacity * sizeof *command);
    if (command == NULL)
        return 1;
    wchar_t *cursor = argument(command, child);
    if (!dap) {
        cursor = argument(cursor, L"-o");
        cursor = argument(cursor, L"command script import scoop_lldb");
    }
    for (int index = 1; index < argc; index++)
        cursor = argument(cursor, argv[index]);
    *cursor = 0;
    STARTUPINFOW startup = {.cb = sizeof startup};
    PROCESS_INFORMATION process;
    SetConsoleCtrlHandler(console_control, TRUE);
    if (!CreateProcessW(child, command, NULL, NULL, TRUE, 0, NULL, NULL, &startup, &process)) {
        fprintf(stderr, "scoop: cannot start debugger (Windows error %lu)\n", GetLastError());
        return 1;
    }
    CloseHandle(process.hThread);
    WaitForSingleObject(process.hProcess, INFINITE);
    DWORD result = 1;
    GetExitCodeProcess(process.hProcess, &result);
    CloseHandle(process.hProcess);
    free(command);
    free(child);
    free(root);
    return (int)result;
}
