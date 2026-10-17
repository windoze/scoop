#define _GNU_SOURCE 1
#include "files.h"

#include <errno.h>
#include <stdbool.h>
#include <sys/stat.h>
#include <unistd.h>

static int file_kind(mode_t mode) {
    if (S_ISREG(mode))
        return 0;
    if (S_ISDIR(mode))
        return 1;
    if (S_ISLNK(mode))
        return 2;
    if (S_ISBLK(mode))
        return 3;
    if (S_ISCHR(mode))
        return 4;
    if (S_ISFIFO(mode))
        return 5;
    if (S_ISSOCK(mode))
        return 6;
    return 7;
}

ScoopPlatformFileMetadata scoop_platform_file_metadata(int fd, const char *path, bool follow) {
    ScoopPlatformFileMetadata result = {.kind = -1};
    struct stat data;
    int status = fd >= 0 ? fstat(fd, &data) : (follow ? stat(path, &data) : lstat(path, &data));
    if (status < 0)
        return result;
    result.kind = file_kind(data.st_mode);
    result.permissions = data.st_mode & 07777;
    result.size = data.st_size;
#ifdef __APPLE__
    result.accessed_seconds = data.st_atimespec.tv_sec;
    result.accessed_nanos = data.st_atimespec.tv_nsec;
    result.modified_seconds = data.st_mtimespec.tv_sec;
    result.modified_nanos = data.st_mtimespec.tv_nsec;
    result.changed_seconds = data.st_ctimespec.tv_sec;
    result.changed_nanos = data.st_ctimespec.tv_nsec;
    result.created_seconds = data.st_birthtimespec.tv_sec;
    result.created_nanos = data.st_birthtimespec.tv_nsec;
#else
    result.accessed_seconds = data.st_atim.tv_sec;
    result.accessed_nanos = data.st_atim.tv_nsec;
    result.modified_seconds = data.st_mtim.tv_sec;
    result.modified_nanos = data.st_mtim.tv_nsec;
    result.changed_seconds = data.st_ctim.tv_sec;
    result.changed_nanos = data.st_ctim.tv_nsec;
    result.created_nanos = -1;
#endif
    return result;
}
