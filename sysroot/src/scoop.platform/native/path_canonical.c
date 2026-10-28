#define _GNU_SOURCE 1
#include "path_strings.h"
#include <stdlib.h>

#ifdef _WIN32
ScoopPlatformString scoop_platform_path_canonical_result(const void *path) {
    HANDLE handle = CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                                NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL);
    if (handle == INVALID_HANDLE_VALUE)
        return (ScoopPlatformString){NULL, 0, (int32_t)GetLastError()};
    DWORD capacity = 256;
    WCHAR *buffer = NULL;
    for (;;) {
        WCHAR *resized = realloc(buffer, (size_t)capacity * sizeof(WCHAR));
        if (resized == NULL) {
            free(buffer);
            CloseHandle(handle);
            return (ScoopPlatformString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
        }
        buffer = resized;
        DWORD count = GetFinalPathNameByHandleW(handle, buffer, capacity,
                                                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
        if (count == 0) {
            DWORD error = GetLastError();
            free(buffer);
            CloseHandle(handle);
            return (ScoopPlatformString){NULL, 0, (int32_t)error};
        }
        if (count < capacity) {
            CloseHandle(handle);
            return (ScoopPlatformString){buffer, (int64_t)count, 0};
        }
        capacity = count;
    }
}
#else
#include <errno.h>
#include <string.h>

ScoopPlatformString scoop_platform_path_canonical_result(const void *path) {
    char *resolved = realpath(path, NULL);
    if (resolved == NULL)
        return (ScoopPlatformString){NULL, 0, errno};
    return (ScoopPlatformString){resolved, (int64_t)strlen(resolved), 0};
}
#endif
