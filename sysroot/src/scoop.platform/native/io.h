#ifndef SCOOP_PLATFORM_IO_H
#define SCOOP_PLATFORM_IO_H

#include <stdint.h>

int64_t scoop_platform_fd_read(int fd, void *data, int64_t count);
int64_t scoop_platform_fd_write(int fd, const void *data, int64_t count);
int scoop_platform_fd_close(int fd);
int scoop_platform_fd_duplicate(int fd);

#endif
