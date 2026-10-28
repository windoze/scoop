#define _GNU_SOURCE 1
#include "path_strings.h"
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <winioctl.h>

ScoopPlatformFileResult scoop_platform_path_symlink_result(const void *link, const void *target,
                                                           bool directory) {
    DWORD flags = directory ? SYMBOLIC_LINK_FLAG_DIRECTORY : 0;
    if (CreateSymbolicLinkW(link, target, flags | SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE))
        return (ScoopPlatformFileResult){0, 0};
    DWORD error = GetLastError();
    if (error == ERROR_INVALID_PARAMETER)
        error = CreateSymbolicLinkW(link, target, flags) ? 0 : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformPathString scoop_platform_path_readlink_result(const void *path) {
    HANDLE handle =
        CreateFileW(path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL,
                    OPEN_EXISTING, FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS, NULL);
    if (handle == INVALID_HANDLE_VALUE)
        return (ScoopPlatformPathString){NULL, 0, (int32_t)GetLastError()};
    unsigned char bytes[MAXIMUM_REPARSE_DATA_BUFFER_SIZE];
    DWORD received = 0;
    BOOL success = DeviceIoControl(handle, FSCTL_GET_REPARSE_POINT, NULL, 0, bytes, sizeof(bytes),
                                   &received, NULL);
    DWORD error = success ? 0 : GetLastError();
    CloseHandle(handle);
    if (error != 0)
        return (ScoopPlatformPathString){NULL, 0, (int32_t)error};

    /* REPARSE_DATA_BUFFER is a driver header type. Read its documented fields without
       depending on that header or on the alignment of the returned bytes. */
    if (received < 8)
        return (ScoopPlatformPathString){NULL, 0, ERROR_INVALID_REPARSE_DATA};
    DWORD tag;
    WORD data_length;
    memcpy(&tag, bytes, sizeof(tag));
    memcpy(&data_length, bytes + 4, sizeof(data_length));
    size_t path_offset;
    if (tag == IO_REPARSE_TAG_SYMLINK)
        path_offset = 20;
    else if (tag == IO_REPARSE_TAG_MOUNT_POINT)
        path_offset = 16;
    else
        return (ScoopPlatformPathString){NULL, 0, ERROR_REPARSE_TAG_MISMATCH};
    size_t data_end = 8 + (size_t)data_length;
    if (data_end > received || data_end < path_offset)
        return (ScoopPlatformPathString){NULL, 0, ERROR_INVALID_REPARSE_DATA};
    WORD offset, length;
    memcpy(&offset, bytes + 8, sizeof(offset));
    memcpy(&length, bytes + 10, sizeof(length));
    if ((offset | length) % sizeof(WCHAR) != 0 || (size_t)offset + length > data_end - path_offset)
        return (ScoopPlatformPathString){NULL, 0, ERROR_INVALID_REPARSE_DATA};
    DWORD flags = 0;
    if (tag == IO_REPARSE_TAG_SYMLINK)
        memcpy(&flags, bytes + 16, sizeof(flags));
    WCHAR *result = malloc((size_t)length + sizeof(WCHAR));
    if (result == NULL)
        return (ScoopPlatformPathString){NULL, 0, ERROR_NOT_ENOUGH_MEMORY};
    memcpy(result, bytes + path_offset + offset, length);
    size_t units = length / sizeof(WCHAR);
    result[units] = L'\0';
    /* Absolute substitute names use the NT namespace. Keep an equivalent Win32 path. */
    if (!(flags & 1) && units >= 4 && result[0] == L'\\' && result[1] == L'?' &&
        result[2] == L'?' && result[3] == L'\\')
        result[1] = L'\\';
    return (ScoopPlatformPathString){result, (int64_t)units, 0};
}
#else
#include <errno.h>
#include <unistd.h>

ScoopPlatformFileResult scoop_platform_path_symlink_result(const void *link, const void *target,
                                                           bool directory) {
    (void)directory;
    int status = symlink(target, link);
    return (ScoopPlatformFileResult){status, status < 0 ? errno : 0};
}

ScoopPlatformPathString scoop_platform_path_readlink_result(const void *path) {
    size_t capacity = 256;
    char *buffer = NULL;
    for (;;) {
        char *resized = realloc(buffer, capacity);
        if (resized == NULL) {
            int error = errno;
            free(buffer);
            return (ScoopPlatformPathString){NULL, 0, error};
        }
        buffer = resized;
        ssize_t count = readlink(path, buffer, capacity);
        if (count < 0) {
            int error = errno;
            free(buffer);
            return (ScoopPlatformPathString){NULL, 0, error};
        }
        if ((size_t)count < capacity)
            return (ScoopPlatformPathString){buffer, (int64_t)count, 0};
        if (capacity > (size_t)PTRDIFF_MAX / 2) {
            free(buffer);
            return (ScoopPlatformPathString){NULL, 0, ENOMEM};
        }
        capacity *= 2;
    }
}
#endif
