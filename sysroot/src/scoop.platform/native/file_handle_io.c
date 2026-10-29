#define _GNU_SOURCE 1
#include "file_handles.h"

#ifdef _WIN32
static DWORD transfer_size(int64_t count) {
    return (uint64_t)count > UINT32_MAX ? UINT32_MAX : (DWORD)count;
}

ScoopPlatformFileResult scoop_platform_handle_read(ScoopPlatformFileHandle handle, void *data,
                                                   int64_t count) {
    if (count < 0)
        return (ScoopPlatformFileResult){-1, ERROR_INVALID_PARAMETER};
    if (count == 0)
        return (ScoopPlatformFileResult){0, 0};
    DWORD transferred = 0;
    if (!ReadFile(scoop_file_handle(handle), data, transfer_size(count), &transferred, NULL)) {
        DWORD error = GetLastError();
        if (error == ERROR_BROKEN_PIPE || error == ERROR_HANDLE_EOF)
            return (ScoopPlatformFileResult){0, 0};
        return (ScoopPlatformFileResult){-1, (int32_t)error};
    }
    return (ScoopPlatformFileResult){transferred, 0};
}

ScoopPlatformFileResult scoop_platform_handle_write(ScoopPlatformFileHandle handle,
                                                    const void *data, int64_t count) {
    if (count < 0)
        return (ScoopPlatformFileResult){-1, ERROR_INVALID_PARAMETER};
    if (count == 0)
        return (ScoopPlatformFileResult){0, 0};
    DWORD transferred = 0;
    if (!WriteFile(scoop_file_handle(handle), data, transfer_size(count), &transferred, NULL))
        return (ScoopPlatformFileResult){-1, (int32_t)GetLastError()};
    return (ScoopPlatformFileResult){transferred, 0};
}

ScoopPlatformPipeOpened scoop_platform_handle_pipe(void) {
    HANDLE source, sink;
    if (!CreatePipe(&source, &sink, NULL, 0))
        return (ScoopPlatformPipeOpened){{UINT64_MAX}, {UINT64_MAX}, (int32_t)GetLastError()};
    return (ScoopPlatformPipeOpened){scoop_file_wrap(source), scoop_file_wrap(sink), 0};
}
#else
#include "io.h"
#include "process_native.h"
#include <errno.h>

ScoopPlatformFileResult scoop_platform_handle_read(ScoopPlatformFileHandle handle, void *data,
                                                   int64_t count) {
    int64_t result = scoop_platform_fd_read(scoop_file_handle(handle), data, count);
    return (ScoopPlatformFileResult){result, result < 0 ? errno : 0};
}

ScoopPlatformFileResult scoop_platform_handle_write(ScoopPlatformFileHandle handle,
                                                    const void *data, int64_t count) {
    int64_t result = scoop_platform_fd_write(scoop_file_handle(handle), data, count);
    return (ScoopPlatformFileResult){result, result < 0 ? errno : 0};
}

ScoopPlatformPipeOpened scoop_platform_handle_pipe(void) {
    int32_t descriptors[2];
    if (scoop_platform_pipe_open(descriptors) < 0)
        return (ScoopPlatformPipeOpened){{UINT64_MAX}, {UINT64_MAX}, errno};
    return (ScoopPlatformPipeOpened){scoop_file_wrap(descriptors[0]),
                                     scoop_file_wrap(descriptors[1]), 0};
}
#endif
