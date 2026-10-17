#define _GNU_SOURCE 1
#include <dirent.h>
#include <errno.h>
#include <string.h>

void *scoop_platform_directory_open(const char *path) { return opendir(path); }
int scoop_platform_directory_close(void *directory) { return closedir(directory); }

const char *scoop_platform_directory_next(void *directory) {
    for (;;) {
        errno = 0;
        struct dirent *entry = readdir(directory);
        if (entry == NULL)
            return NULL;
        if (strcmp(entry->d_name, ".") != 0 && strcmp(entry->d_name, "..") != 0)
            return entry->d_name;
    }
}
