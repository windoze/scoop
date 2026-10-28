#define _GNU_SOURCE 1
#include "paths.h"
#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#ifndef __APPLE__
#include <sys/syscall.h>
#endif

int scoop_platform_path_mkdir(const char *path, int mode) { return mkdir(path, (mode_t)mode); }
int scoop_platform_path_delete(const char *path) { return remove(path); }

int scoop_platform_path_move(const char *source, const char *destination, bool replace) {
    if (replace)
        return rename(source, destination);
#ifdef __APPLE__
    return renamex_np(source, destination, RENAME_EXCL);
#else
    return (int)syscall(SYS_renameat2, AT_FDCWD, source, AT_FDCWD, destination, RENAME_NOREPLACE);
#endif
}

int scoop_platform_file_temporary(char *pattern) { return mkostemp(pattern, O_CLOEXEC); }

int scoop_platform_path_temporary(char *pattern, bool directory) {
    if (directory)
        return mkdtemp(pattern) == NULL ? -1 : 0;
    int fd = scoop_platform_file_temporary(pattern);
    if (fd < 0)
        return -1;
    /* No descriptor escapes this operation; close once, and remove on failure. */
    if (close(fd) == 0)
        return 0;
    int saved = errno;
    (void)unlink(pattern);
    errno = saved;
    return -1;
}
