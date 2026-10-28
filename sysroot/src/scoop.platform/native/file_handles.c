#define _GNU_SOURCE 1
#include "file_handles.h"
#include <string.h>

#ifdef _WIN32

static ScoopPlatformFileOpened open_handle(const void *path, int32_t flags, int32_t mode,
                                           DWORD attributes) {
    (void)mode;
    DWORD access = (flags & 1) ? GENERIC_READ : 0;
    if (flags & 2)
        access |= (flags & 4) ? (FILE_GENERIC_WRITE & ~FILE_WRITE_DATA) : GENERIC_WRITE;
    DWORD creation = (flags & 16)  ? CREATE_NEW
                     : (flags & 8) ? ((flags & 32) ? CREATE_ALWAYS : OPEN_ALWAYS)
                                   : ((flags & 32) ? TRUNCATE_EXISTING : OPEN_EXISTING);
    HANDLE handle =
        CreateFileW(path, access, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL,
                    creation, attributes, NULL);
    DWORD error = handle == INVALID_HANDLE_VALUE ? GetLastError() : 0;
    return (ScoopPlatformFileOpened){scoop_file_wrap(handle), (int32_t)error};
}

ScoopPlatformFileOpened scoop_platform_handle_open(const void *path, int32_t flags, int32_t mode) {
    return open_handle(path, flags, mode, FILE_ATTRIBUTE_NORMAL);
}

ScoopPlatformFileOpened scoop_platform_handle_open_overlapped(const void *path, int32_t flags,
                                                              int32_t mode) {
    return open_handle(path, flags, mode, FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED);
}

ScoopPlatformFileOpened scoop_platform_handle_duplicate(ScoopPlatformFileHandle handle) {
    HANDLE copy = INVALID_HANDLE_VALUE;
    HANDLE process = GetCurrentProcess();
    DWORD error = DuplicateHandle(process, scoop_file_handle(handle), process, &copy, 0, FALSE,
                                  DUPLICATE_SAME_ACCESS)
                      ? 0
                      : GetLastError();
    return (ScoopPlatformFileOpened){scoop_file_wrap(copy), (int32_t)error};
}

ScoopPlatformFileOpened scoop_platform_handle_standard(int32_t stream) {
    DWORD which = stream == 0   ? STD_INPUT_HANDLE
                  : stream == 1 ? STD_OUTPUT_HANDLE
                                : STD_ERROR_HANDLE;
    HANDLE handle = GetStdHandle(which);
    DWORD error = handle == INVALID_HANDLE_VALUE ? GetLastError()
                  : handle == NULL               ? ERROR_INVALID_HANDLE
                                                 : 0;
    return (ScoopPlatformFileOpened){scoop_file_wrap(handle), (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_handle_close(ScoopPlatformFileHandle handle) {
    DWORD error = CloseHandle(scoop_file_handle(handle)) ? 0 : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_handle_seek(ScoopPlatformFileHandle handle, int64_t offset,
                                                   bool current) {
    LARGE_INTEGER distance, position;
    distance.QuadPart = offset;
    if (!SetFilePointerEx(scoop_file_handle(handle), distance, &position,
                          current ? FILE_CURRENT : FILE_BEGIN))
        return (ScoopPlatformFileResult){-1, (int32_t)GetLastError()};
    return (ScoopPlatformFileResult){position.QuadPart, 0};
}

ScoopPlatformFileResult scoop_platform_handle_truncate(ScoopPlatformFileHandle handle,
                                                       int64_t size) {
    FILE_END_OF_FILE_INFO info;
    info.EndOfFile.QuadPart = size;
    DWORD error = SetFileInformationByHandle(scoop_file_handle(handle), FileEndOfFileInfo, &info,
                                             sizeof(info))
                      ? 0
                      : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_handle_sync(ScoopPlatformFileHandle handle, bool data_only) {
    (void)data_only;
    DWORD error = FlushFileBuffers(scoop_file_handle(handle)) ? 0 : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_handle_same(ScoopPlatformFileHandle first,
                                                   ScoopPlatformFileHandle second) {
    FILE_ID_INFO left, right;
    if (!GetFileInformationByHandleEx(scoop_file_handle(first), FileIdInfo, &left, sizeof(left)) ||
        !GetFileInformationByHandleEx(scoop_file_handle(second), FileIdInfo, &right, sizeof(right)))
        return (ScoopPlatformFileResult){-1, (int32_t)GetLastError()};
    bool same = left.VolumeSerialNumber == right.VolumeSerialNumber &&
                memcmp(left.FileId.Identifier, right.FileId.Identifier,
                       sizeof(left.FileId.Identifier)) == 0;
    return (ScoopPlatformFileResult){same, 0};
}

ScoopPlatformFileResult scoop_platform_handle_lock(ScoopPlatformFileHandle handle,
                                                   int32_t operation) {
    OVERLAPPED position = {0};
    DWORD flags = (operation & 1) ? 0 : LOCKFILE_EXCLUSIVE_LOCK;
    if (operation & 4)
        flags |= LOCKFILE_FAIL_IMMEDIATELY;
    BOOL success =
        operation == 0
            ? UnlockFileEx(scoop_file_handle(handle), 0, UINT32_MAX, UINT32_MAX, &position)
            : LockFileEx(scoop_file_handle(handle), flags, 0, UINT32_MAX, UINT32_MAX, &position);
    DWORD error = success ? 0 : GetLastError();
    if ((operation & 4) && error == ERROR_LOCK_VIOLATION)
        return (ScoopPlatformFileResult){1, 0};
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

#else
#include "io.h"
#include <errno.h>

static ScoopPlatformFileResult file_result(int64_t value) {
    return (ScoopPlatformFileResult){value, value < 0 ? errno : 0};
}

ScoopPlatformFileOpened scoop_platform_handle_open(const void *path, int32_t flags, int32_t mode) {
    int fd = scoop_platform_file_open(path, flags, mode);
    return (ScoopPlatformFileOpened){scoop_file_wrap(fd), fd < 0 ? errno : 0};
}

ScoopPlatformFileOpened scoop_platform_handle_duplicate(ScoopPlatformFileHandle handle) {
    int fd = scoop_platform_fd_duplicate(scoop_file_handle(handle));
    return (ScoopPlatformFileOpened){scoop_file_wrap(fd), fd < 0 ? errno : 0};
}

ScoopPlatformFileOpened scoop_platform_handle_standard(int32_t stream) {
    return (ScoopPlatformFileOpened){scoop_file_wrap(stream), 0};
}

ScoopPlatformFileResult scoop_platform_handle_close(ScoopPlatformFileHandle handle) {
    return file_result(scoop_platform_fd_close(scoop_file_handle(handle)));
}

ScoopPlatformFileResult scoop_platform_handle_seek(ScoopPlatformFileHandle handle, int64_t offset,
                                                   bool current) {
    return file_result(scoop_platform_file_seek(scoop_file_handle(handle), offset, current));
}

ScoopPlatformFileResult scoop_platform_handle_truncate(ScoopPlatformFileHandle handle,
                                                       int64_t size) {
    return file_result(scoop_platform_file_truncate(scoop_file_handle(handle), size));
}

ScoopPlatformFileResult scoop_platform_handle_sync(ScoopPlatformFileHandle handle, bool data_only) {
    return file_result(scoop_platform_file_sync(scoop_file_handle(handle), data_only));
}

ScoopPlatformFileResult scoop_platform_handle_same(ScoopPlatformFileHandle first,
                                                   ScoopPlatformFileHandle second) {
    return file_result(
        scoop_platform_file_same(scoop_file_handle(first), scoop_file_handle(second)));
}

ScoopPlatformFileResult scoop_platform_handle_lock(ScoopPlatformFileHandle handle,
                                                   int32_t operation) {
    return file_result(scoop_platform_file_lock(scoop_file_handle(handle), operation));
}
#endif
