#ifndef SCOOP_PLATFORM_PATH_STRINGS_H
#define SCOOP_PLATFORM_PATH_STRINGS_H

#include "file_handles.h"

/* Owned malloc storage: UTF-16 units on Windows, bytes on POSIX. */
typedef struct {
    void *data;
    int64_t length;
    int32_t error;
} ScoopPlatformPathString;

ScoopPlatformFileResult scoop_platform_path_symlink_result(const void *link, const void *target,
                                                           bool directory);
ScoopPlatformPathString scoop_platform_path_readlink_result(const void *path);
ScoopPlatformPathString scoop_platform_path_canonical_result(const void *path);
ScoopPlatformPathString scoop_platform_temporary_directory_result(void);

#endif
