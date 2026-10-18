#ifndef SCOOP_PLATFORM_PATHS_H
#define SCOOP_PLATFORM_PATHS_H

#include <stdbool.h>

int scoop_platform_path_mkdir(const char *path, int mode);
int scoop_platform_path_delete(const char *path);
int scoop_platform_path_move(const char *source, const char *destination, bool replace);
int scoop_platform_file_temporary(char *pattern);

#endif
