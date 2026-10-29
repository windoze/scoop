#include "process_windows.h"
#include <stdlib.h>

struct SpawnHandles {
    HANDLE child[3];
    HANDLE parent[3];
};

static void close_handles(struct SpawnHandles *handles, bool success) {
    for (int index = 0; index < 3; ++index) {
        if (handles->child[index] != NULL)
            (void)CloseHandle(handles->child[index]);
        if (!success && handles->parent[index] != INVALID_HANDLE_VALUE)
            (void)CloseHandle(handles->parent[index]);
    }
}

static DWORD inherit_handle(HANDLE source, HANDLE *result) {
    if (DuplicateHandle(GetCurrentProcess(), source, GetCurrentProcess(), result, 0, TRUE,
                        DUPLICATE_SAME_ACCESS))
        return 0;
    return GetLastError();
}

static DWORD redirect(struct SpawnHandles *handles, int index, ScoopPlatformStdio source) {
    if (source.kind == 0) {
        static const DWORD streams[] = {STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
        HANDLE inherited = GetStdHandle(streams[index]);
        if (inherited == NULL || inherited == INVALID_HANDLE_VALUE)
            return 0;
        return inherit_handle(inherited, &handles->child[index]);
    }
    if (source.kind == 1) {
        SECURITY_ATTRIBUTES attributes = {sizeof(attributes), NULL, TRUE};
        HANDLE handle = CreateFileW(L"NUL", index == 0 ? GENERIC_READ : GENERIC_WRITE,
                                    FILE_SHARE_READ | FILE_SHARE_WRITE, &attributes, OPEN_EXISTING,
                                    FILE_ATTRIBUTE_NORMAL, NULL);
        if (handle == INVALID_HANDLE_VALUE)
            return GetLastError();
        handles->child[index] = handle;
        return 0;
    }
    if (source.kind == 4)
        return handles->child[1] == NULL
                   ? 0
                   : inherit_handle(handles->child[1], &handles->child[index]);
    if (source.kind == 2) {
        HANDLE reader, writer;
        if (!CreatePipe(&reader, &writer, NULL, 0))
            return GetLastError();
        HANDLE child = index == 0 ? reader : writer;
        handles->parent[index] = index == 0 ? writer : reader;
        DWORD error = inherit_handle(child, &handles->child[index]);
        (void)CloseHandle(child);
        return error;
    }
    return inherit_handle(scoop_file_handle(source.handle), &handles->child[index]);
}

ScoopPlatformSpawnResult scoop_platform_process_spawn(
    const WCHAR *executable, const WCHAR *arguments, int64_t argument_count,
    const WCHAR *environment, int64_t environment_count, const WCHAR *directory,
    const ScoopPlatformStdio redirects[3], ScoopPlatformFileHandle result[3]) {
    DWORD error = 0;
    WCHAR *command = scoop_process_command_line(arguments, argument_count, &error);
    if (command == NULL)
        return scoop_process_failure((int32_t)error);
    WCHAR *block = scoop_process_environment(environment, environment_count, &error);
    WCHAR *application = block == NULL ? NULL : scoop_process_absolute_path(executable, &error);
    WCHAR *cwd = application == NULL ? NULL : scoop_process_absolute_path(directory, &error);
    struct SpawnHandles handles = {
        {NULL, NULL, NULL}, {INVALID_HANDLE_VALUE, INVALID_HANDLE_VALUE, INVALID_HANDLE_VALUE}};
    for (int index = 0; index < 3 && error == 0; ++index)
        error = redirect(&handles, index, redirects[index]);

    STARTUPINFOEXW startup = {0};
    startup.StartupInfo.cb = sizeof(STARTUPINFOW);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = handles.child[0];
    startup.StartupInfo.hStdOutput = handles.child[1];
    startup.StartupInfo.hStdError = handles.child[2];
    HANDLE inherited[3];
    SIZE_T count = 0;
    for (int index = 0; index < 3; ++index) {
        if (handles.child[index] != NULL)
            inherited[count++] = handles.child[index];
    }
    bool initialized = false;
    DWORD flags = CREATE_UNICODE_ENVIRONMENT;
    if (error == 0 && count != 0) {
        SIZE_T size = 0;
        (void)InitializeProcThreadAttributeList(NULL, 1, 0, &size);
        if (size == 0) {
            error = GetLastError();
        } else {
            startup.lpAttributeList = malloc(size);
            if (startup.lpAttributeList == NULL)
                error = ERROR_NOT_ENOUGH_MEMORY;
            else if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &size))
                error = GetLastError();
            else {
                initialized = true;
                if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0,
                                               PROC_THREAD_ATTRIBUTE_HANDLE_LIST, inherited,
                                               count * sizeof(HANDLE), NULL, NULL))
                    error = GetLastError();
            }
        }
        flags |= EXTENDED_STARTUPINFO_PRESENT;
        startup.StartupInfo.cb = sizeof(startup);
    }
    PROCESS_INFORMATION process = {0};
    if (error == 0 && !CreateProcessW(application, command, NULL, NULL, count != 0, flags, block,
                                      cwd, &startup.StartupInfo, &process))
        error = GetLastError();
    if (initialized)
        DeleteProcThreadAttributeList(startup.lpAttributeList);
    free(startup.lpAttributeList);
    close_handles(&handles, error == 0);
    free(cwd);
    free(application);
    free(block);
    free(command);
    if (error != 0)
        return scoop_process_failure((int32_t)error);
    (void)CloseHandle(process.hThread);
    for (int index = 0; index < 3; ++index)
        result[index] = scoop_file_wrap(handles.parent[index]);
    return (ScoopPlatformSpawnResult){{(uint64_t)(uintptr_t)process.hProcess}, 0};
}
