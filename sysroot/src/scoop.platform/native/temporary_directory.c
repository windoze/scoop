#define _GNU_SOURCE 1
#include "path_strings.h"
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
ScoopPlatformPathString scoop_platform_temporary_directory_result(void) {
    typedef DWORD(WINAPI * TempPath)(DWORD, LPWSTR);
    FARPROC entry = GetProcAddress(GetModuleHandleW(L"kernel32.dll"), "GetTempPath2W");
    TempPath query = GetTempPathW;
    if (entry != NULL) {
        _Static_assert(sizeof(query) == sizeof(entry), "Windows function pointer size");
        memcpy(&query, &entry, sizeof(query));
    }
    DWORD capacity = MAX_PATH + 1;
    WCHAR *buffer = NULL;
    for (;;) {
        WCHAR *resized = realloc(buffer, (size_t)capacity * sizeof(WCHAR));
        if (resized == NULL) {
            free(buffer);
            return (ScoopPlatformPathString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
        }
        buffer = resized;
        DWORD count = query(capacity, buffer);
        if (count == 0) {
            DWORD error = GetLastError();
            free(buffer);
            return (ScoopPlatformPathString){NULL, 0, (int32_t)error};
        }
        if (count < capacity)
            return (ScoopPlatformPathString){buffer, (int64_t)count, 0};
        capacity = count;
    }
}
#else
#include <errno.h>

ScoopPlatformPathString scoop_platform_temporary_directory_result(void) {
    const char *configured = getenv("TMPDIR");
    const char *path = configured != NULL && configured[0] != '\0' ? configured : "/tmp";
    char *copy = strdup(path);
    if (copy == NULL)
        return (ScoopPlatformPathString){NULL, 0, errno};
    return (ScoopPlatformPathString){copy, (int64_t)strlen(copy), 0};
}
#endif
