#ifndef SCOOP_PLATFORM_PATHS_H
#define SCOOP_PLATFORM_PATHS_H

#include "file_handles.h"

ScoopPlatformFileResult scoop_platform_path_mkdir_result(const void *path, int32_t mode);
ScoopPlatformFileResult scoop_platform_path_delete_result(const void *path);
ScoopPlatformFileResult scoop_platform_path_move_result(const void *source, const void *destination,
                                                        bool replace);

int scoop_platform_path_mkdir(const char *path, int mode);
int scoop_platform_path_delete(const char *path);
int scoop_platform_path_move(const char *source, const char *destination, bool replace);
int scoop_platform_file_temporary(char *pattern);

#endif
