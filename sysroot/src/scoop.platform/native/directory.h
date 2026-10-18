#ifndef SCOOP_PLATFORM_DIRECTORY_H
#define SCOOP_PLATFORM_DIRECTORY_H

void *scoop_platform_directory_open(const char *path);
const char *scoop_platform_directory_next(void *directory);
int scoop_platform_directory_close(void *directory);

#endif
