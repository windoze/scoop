#define _GNU_SOURCE 1
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdbool.h>
#include <stdint.h>
#include <sys/stat.h>
#include <unistd.h>

int scoop_platform_file_open(const char *path, int flags, int mode) {
    int native = O_CLOEXEC;
    native |= (flags & 1) ? ((flags & 2) ? O_RDWR : O_RDONLY) : O_WRONLY;
    if (flags & 4)
        native |= O_APPEND;
    if (flags & 8)
        native |= O_CREAT;
    if (flags & 16)
        native |= O_CREAT | O_EXCL;
    if (flags & 32)
        native |= O_TRUNC;
    int result;
    do {
        result = open(path, native, (mode_t)mode);
    } while (result < 0 && errno == EINTR);
    return result;
}

int64_t scoop_platform_file_seek(int fd, int64_t offset, bool current) {
    off_t converted = (off_t)offset;
    if ((int64_t)converted != offset || offset < 0) {
        errno = EOVERFLOW;
        return -1;
    }
    return (int64_t)lseek(fd, converted, current ? SEEK_CUR : SEEK_SET);
}

int scoop_platform_file_truncate(int fd, int64_t size) {
    off_t converted = (off_t)size;
    if ((int64_t)converted != size || size < 0) {
        errno = EOVERFLOW;
        return -1;
    }
    int result;
    do {
        result = ftruncate(fd, converted);
    } while (result < 0 && errno == EINTR);
    return result;
}

int scoop_platform_file_sync(int fd, bool data_only) {
    int result;
    do {
#ifdef __APPLE__
        /* fsync is a valid stronger implementation of data-only sync. */
        (void)data_only;
        result = fsync(fd);
#else
        result = data_only ? fdatasync(fd) : fsync(fd);
#endif
    } while (result < 0 && errno == EINTR);
    return result;
}

int scoop_platform_file_same(int first, int second) {
    struct stat left, right;
    if (fstat(first, &left) < 0 || fstat(second, &right) < 0)
        return -1;
    return left.st_dev == right.st_dev && left.st_ino == right.st_ino;
}
