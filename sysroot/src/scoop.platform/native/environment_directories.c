#define _GNU_SOURCE 1
#include "environment.h"
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <userenv.h>
#include <wchar.h>

ScoopPlatformString scoop_platform_current_directory_result(void) {
    DWORD capacity = 256;
    WCHAR *buffer = NULL;
    for (;;) {
        WCHAR *resized = realloc(buffer, (size_t)capacity * sizeof(WCHAR));
        if (resized == NULL) {
            free(buffer);
            return (ScoopPlatformString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
        }
        buffer = resized;
        DWORD count = GetCurrentDirectoryW(capacity, buffer);
        if (count == 0) {
            DWORD error = GetLastError();
            free(buffer);
            return (ScoopPlatformString){NULL, 0, (int32_t)error};
        }
        if (count < capacity)
            return (ScoopPlatformString){buffer, (int64_t)count, 0};
        capacity = count;
    }
}

ScoopPlatformString scoop_platform_home_directory_result(void) {
    ScoopPlatformString configured = scoop_platform_environment_value_result(L"USERPROFILE");
    if (configured.error != 0 || configured.length != 0)
        return configured;
    free(configured.data);
    HANDLE token;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token))
        return (ScoopPlatformString){NULL, 0, (int32_t)GetLastError()};
    DWORD capacity = 256;
    WCHAR *buffer = NULL;
    for (;;) {
        WCHAR *resized = realloc(buffer, (size_t)capacity * sizeof(WCHAR));
        if (resized == NULL) {
            free(buffer);
            CloseHandle(token);
            return (ScoopPlatformString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
        }
        buffer = resized;
        DWORD count = capacity;
        if (GetUserProfileDirectoryW(token, buffer, &count)) {
            CloseHandle(token);
            size_t length = wcslen(buffer);
            if (length == 0) {
                free(buffer);
                return (ScoopPlatformString){NULL, 0, 0};
            }
            return (ScoopPlatformString){buffer, (int64_t)length, 0};
        }
        DWORD error = GetLastError();
        if (error != ERROR_INSUFFICIENT_BUFFER) {
            free(buffer);
            CloseHandle(token);
            return (ScoopPlatformString){NULL, 0, (int32_t)error};
        }
        capacity = count;
    }
}
#else
#include <errno.h>
#include <pwd.h>
#include <unistd.h>

ScoopPlatformString scoop_platform_current_directory_result(void) {
    char *path = getcwd(NULL, 0);
    if (path == NULL)
        return (ScoopPlatformString){NULL, 0, errno};
    return (ScoopPlatformString){path, (int64_t)strlen(path), 0};
}

ScoopPlatformString scoop_platform_home_directory_result(void) {
    ScoopPlatformString configured = scoop_platform_environment_value_result("HOME");
    if (configured.error != 0 || configured.length != 0)
        return configured;
    free(configured.data);
    long suggested = sysconf(_SC_GETPW_R_SIZE_MAX);
    size_t size = suggested > 0 ? (size_t)suggested : 1024;
    for (;;) {
        char *buffer = malloc(size);
        if (buffer == NULL)
            return (ScoopPlatformString){NULL, 0, errno};
        struct passwd entry, *result;
        int error = getpwuid_r(getuid(), &entry, buffer, size, &result);
        if (error == ERANGE) {
            free(buffer);
            if (size > SIZE_MAX / 2)
                return (ScoopPlatformString){NULL, 0, EOVERFLOW};
            size *= 2;
            continue;
        }
        char *home = NULL;
        if (error == 0 && result != NULL && result->pw_dir != NULL && result->pw_dir[0] != '\0') {
            home = strdup(result->pw_dir);
            if (home == NULL)
                error = errno;
        }
        free(buffer);
        return (ScoopPlatformString){home, home == NULL ? 0 : (int64_t)strlen(home), error};
    }
}
#endif
