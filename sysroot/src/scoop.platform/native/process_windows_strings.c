#include "process_windows.h"
#include <limits.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

/* CreateProcessW accepts at most 32767 UTF-16 units, including the terminator. */
static bool emit(WCHAR *output, size_t *position, WCHAR value, size_t count) {
    if (count > 32766 - *position)
        return false;
    if (output != NULL) {
        for (size_t index = 0; index < count; ++index)
            output[*position + index] = value;
    }
    *position += count;
    return true;
}

static bool quote(WCHAR *output, size_t *position, const WCHAR *argument, bool first) {
    if (!emit(output, position, L'"', 1))
        return false;
    while (*argument != 0) {
        if (first) {
            if (*argument == L'"' || !emit(output, position, *argument++, 1))
                return false;
            continue;
        }
        size_t slashes = 0;
        while (*argument == L'\\') {
            ++slashes;
            ++argument;
            if (slashes > 32766)
                return false;
        }
        size_t count = (*argument == L'"' || *argument == 0) ? slashes * 2 : slashes;
        if (!emit(output, position, L'\\', count))
            return false;
        if (*argument == L'"' && !emit(output, position, L'\\', 1))
            return false;
        if (*argument != 0 && !emit(output, position, *argument++, 1))
            return false;
    }
    return emit(output, position, L'"', 1);
}

WCHAR *scoop_process_command_line(const WCHAR *arguments, int64_t count, DWORD *error) {
    if (count <= 0) {
        *error = ERROR_INVALID_PARAMETER;
        return NULL;
    }
    WCHAR *output = NULL;
    for (int pass = 0; pass < 2; ++pass) {
        size_t position = 0;
        const WCHAR *argument = arguments;
        for (int64_t index = 0; index < count; ++index) {
            if ((index != 0 && !emit(output, &position, L' ', 1)) ||
                !quote(output, &position, argument, index == 0)) {
                *error = ERROR_INVALID_PARAMETER;
                free(output);
                return NULL;
            }
            argument += wcslen(argument) + 1;
        }
        if (output != NULL) {
            output[position] = 0;
            return output;
        }
        output = malloc((position + 1) * sizeof(*output));
        if (output == NULL) {
            *error = ERROR_NOT_ENOUGH_MEMORY;
            return NULL;
        }
    }
    return output;
}

static int compare_environment(const void *left, const void *right) {
    const WCHAR *first = *(const WCHAR *const *)left;
    const WCHAR *second = *(const WCHAR *const *)right;
    return CompareStringOrdinal(first, (int)(wcschr(first, L'=') - first), second,
                                (int)(wcschr(second, L'=') - second), TRUE) -
           CSTR_EQUAL;
}

WCHAR *scoop_process_environment(const WCHAR *environment, int64_t count, DWORD *error) {
    if (count < 0 || (uint64_t)count > SIZE_MAX / sizeof(WCHAR *)) {
        *error = ERROR_ARITHMETIC_OVERFLOW;
        return NULL;
    }
    const WCHAR **entries = count == 0 ? NULL : malloc((size_t)count * sizeof(*entries));
    if (count != 0 && entries == NULL) {
        *error = ERROR_NOT_ENOUGH_MEMORY;
        return NULL;
    }
    size_t units = count == 0 ? 2 : 1;
    for (int64_t index = 0; index < count; ++index) {
        size_t length = wcslen(environment);
        const WCHAR *equals = wcschr(environment, L'=');
        if (equals == NULL || equals == environment || equals - environment > INT_MAX) {
            *error = ERROR_INVALID_PARAMETER;
            free(entries);
            return NULL;
        }
        if (length >= SIZE_MAX / sizeof(WCHAR) - units) {
            *error = ERROR_ARITHMETIC_OVERFLOW;
            free(entries);
            return NULL;
        }
        units += length + 1;
        entries[index] = environment;
        environment += length + 1;
    }
    if (count > 1)
        qsort(entries, (size_t)count, sizeof(*entries), compare_environment);
    WCHAR *output = calloc(units, sizeof(*output));
    if (output == NULL) {
        *error = ERROR_NOT_ENOUGH_MEMORY;
        free(entries);
        return NULL;
    }
    size_t position = 0;
    for (int64_t index = 0; index < count; ++index) {
        if (index > 0 && compare_environment(&entries[index - 1], &entries[index]) == 0) {
            *error = ERROR_INVALID_PARAMETER;
            free(entries);
            free(output);
            return NULL;
        }
        size_t length = wcslen(entries[index]) + 1;
        memcpy(output + position, entries[index], length * sizeof(*output));
        position += length;
    }
    free(entries);
    return output;
}

WCHAR *scoop_process_absolute_path(const WCHAR *path, DWORD *error) {
    DWORD needed = GetFullPathNameW(path, 0, NULL, NULL);
    if (needed == 0) {
        *error = GetLastError();
        return NULL;
    }
    WCHAR *result = malloc((size_t)needed * sizeof(*result));
    if (result == NULL) {
        *error = ERROR_NOT_ENOUGH_MEMORY;
        return NULL;
    }
    DWORD length = GetFullPathNameW(path, needed, result, NULL);
    if (length == 0 || length >= needed) {
        *error = length == 0 ? GetLastError() : ERROR_INSUFFICIENT_BUFFER;
        free(result);
        return NULL;
    }
    return result;
}
