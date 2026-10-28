#define _GNU_SOURCE 1
#include "environment.h"
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <wchar.h>

ScoopPlatformString scoop_platform_environment_value_result(const void *name) {
    DWORD capacity = 128;
    WCHAR *buffer = NULL;
    for (;;) {
        WCHAR *resized = realloc(buffer, (size_t)capacity * sizeof(WCHAR));
        if (resized == NULL) {
            free(buffer);
            return (ScoopPlatformString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
        }
        buffer = resized;
        SetLastError(ERROR_SUCCESS);
        DWORD count = GetEnvironmentVariableW(name, buffer, capacity);
        if (count == 0) {
            DWORD error = GetLastError();
            if (error != ERROR_SUCCESS) {
                free(buffer);
                return (ScoopPlatformString){NULL, 0,
                                             error == ERROR_ENVVAR_NOT_FOUND ? 0 : (int32_t)error};
            }
            buffer[0] = L'\0';
        }
        if (count < capacity)
            return (ScoopPlatformString){buffer, (int64_t)count, 0};
        capacity = count;
    }
}

ScoopPlatformString scoop_platform_environment_result(void) {
    WCHAR *block = GetEnvironmentStringsW();
    if (block == NULL)
        return (ScoopPlatformString){NULL, 0, (int32_t)GetLastError()};
    const WCHAR *end = block;
    while (*end != L'\0')
        end += wcslen(end) + 1;
    size_t length = (size_t)(end - block);
    size_t size = (length + 1) * sizeof(WCHAR);
    WCHAR *copy = malloc(size);
    if (copy != NULL)
        memcpy(copy, block, size);
    FreeEnvironmentStringsW(block);
    if (copy == NULL)
        return (ScoopPlatformString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
    return (ScoopPlatformString){copy, (int64_t)length, 0};
}
#else
#include <errno.h>
#ifdef __APPLE__
#include <crt_externs.h>
#else
extern char **environ;
#endif

ScoopPlatformString scoop_platform_environment_value_result(const void *name) {
    const char *value = getenv(name);
    if (value == NULL)
        return (ScoopPlatformString){NULL, 0, 0};
    char *copy = strdup(value);
    if (copy == NULL)
        return (ScoopPlatformString){NULL, 0, errno};
    return (ScoopPlatformString){copy, (int64_t)strlen(copy), 0};
}

ScoopPlatformString scoop_platform_environment_result(void) {
#ifdef __APPLE__
    char **values = *_NSGetEnviron();
#else
    char **values = environ;
#endif
    size_t length = 0;
    for (char **entry = values; entry != NULL && *entry != NULL; ++entry) {
        size_t size = strlen(*entry) + 1;
        if (size > (size_t)PTRDIFF_MAX - length - 1)
            return (ScoopPlatformString){NULL, 0, EOVERFLOW};
        length += size;
    }
    char *copy = malloc(length + 1);
    if (copy == NULL)
        return (ScoopPlatformString){NULL, 0, errno};
    char *next = copy;
    for (char **entry = values; entry != NULL && *entry != NULL; ++entry) {
        size_t size = strlen(*entry) + 1;
        memcpy(next, *entry, size);
        next += size;
    }
    *next = '\0';
    return (ScoopPlatformString){copy, (int64_t)length, 0};
}
#endif
