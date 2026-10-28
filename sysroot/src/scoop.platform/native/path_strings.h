#ifndef SCOOP_PLATFORM_PATH_STRINGS_H
#define SCOOP_PLATFORM_PATH_STRINGS_H

#include "file_handles.h"
#include "os_strings.h"

ScoopPlatformFileResult scoop_platform_path_symlink_result(const void *link, const void *target,
                                                           bool directory);
ScoopPlatformString scoop_platform_path_readlink_result(const void *path);
ScoopPlatformString scoop_platform_path_canonical_result(const void *path);
ScoopPlatformString scoop_platform_temporary_directory_result(void);

#endif
