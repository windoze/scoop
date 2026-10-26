#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdbool.h>
#include <stdint.h>
#include <time.h>

int scoop_test_host_poll(int descriptor, int timeout) {
    struct pollfd entry = {.fd = descriptor, .events = POLLIN};
    int status;
    do {
        status = poll(&entry, 1, timeout);
    } while (status < 0 && errno == EINTR);
    if (status < 0)
        return -errno;
    if (entry.revents & (POLLERR | POLLNVAL | POLLHUP))
        return -EIO;
    return status;
}

bool scoop_test_descriptor_open(int descriptor) { return fcntl(descriptor, F_GETFD) >= 0; }

int64_t scoop_test_host_clock(int kind) {
    struct timespec value;
    clock_gettime(kind ? CLOCK_PROCESS_CPUTIME_ID : CLOCK_MONOTONIC, &value);
    return (int64_t)value.tv_sec * 1000000000 + value.tv_nsec;
}
