#define _GNU_SOURCE 1
#include <errno.h>
#include <stdint.h>
#include <sys/random.h>

int64_t scoop_platform_entropy(void *data, int64_t count) {
    if (count < 0) {
        errno = EINVAL;
        return -1;
    }
    if (count == 0)
        return 0;
    size_t size = count > 256 ? 256 : (size_t)count;
    int64_t result;
    do {
#ifdef __APPLE__
        result = getentropy(data, size) == 0 ? (int64_t)size : -1;
#else
        result = getrandom(data, size, 0);
#endif
    } while (result < 0 && errno == EINTR);
    if (result == 0) {
        errno = EIO;
        return -1;
    }
    return result;
}
