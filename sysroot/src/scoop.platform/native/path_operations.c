#define _GNU_SOURCE 1
#include "file_handles.h"

#ifdef _WIN32
ScoopPlatformFileResult scoop_platform_path_mkdir_result(const void *path, int32_t mode) {
    (void)mode;
    DWORD error = CreateDirectoryW(path, NULL) ? 0 : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_path_delete_result(const void *path) {
    DWORD attributes = GetFileAttributesW(path);
    if (attributes == INVALID_FILE_ATTRIBUTES)
        return (ScoopPlatformFileResult){-1, (int32_t)GetLastError()};
    BOOL success =
        attributes & FILE_ATTRIBUTE_DIRECTORY ? RemoveDirectoryW(path) : DeleteFileW(path);
    DWORD error = success ? 0 : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}

ScoopPlatformFileResult scoop_platform_path_move_result(const void *source, const void *destination,
                                                        bool replace) {
    DWORD error = MoveFileExW(source, destination, replace ? MOVEFILE_REPLACE_EXISTING : 0)
                      ? 0
                      : GetLastError();
    return (ScoopPlatformFileResult){0, (int32_t)error};
}
#else
#include "paths.h"
#include <errno.h>

ScoopPlatformFileResult scoop_platform_path_mkdir_result(const void *path, int32_t mode) {
    int status = scoop_platform_path_mkdir(path, mode);
    return (ScoopPlatformFileResult){status, status < 0 ? errno : 0};
}

ScoopPlatformFileResult scoop_platform_path_delete_result(const void *path) {
    int status = scoop_platform_path_delete(path);
    return (ScoopPlatformFileResult){status, status < 0 ? errno : 0};
}

ScoopPlatformFileResult scoop_platform_path_move_result(const void *source, const void *destination,
                                                        bool replace) {
    int status = scoop_platform_path_move(source, destination, replace);
    return (ScoopPlatformFileResult){status, status < 0 ? errno : 0};
}
#endif
