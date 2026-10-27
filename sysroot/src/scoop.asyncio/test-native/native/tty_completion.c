#include "../../native/loop.h"
#include <errno.h>
#include <poll.h>
#include <time.h>

/* Observe the real logical completion without delivering it to the managed operation. */
int scoop_test_tty_completion_ready(ScoopIoLoop *loop) {
    struct timespec start;
    clock_gettime(CLOCK_MONOTONIC, &start);
    for (;;) {
        int status = scoop_io_loop_poll(loop, false);
        if (status < 0)
            return status;
        if (scoop_io_loop_batch_size(loop))
            return 0;
        struct timespec now;
        clock_gettime(CLOCK_MONOTONIC, &now);
        int64_t elapsed =
            (now.tv_sec - start.tv_sec) * 1000 + (now.tv_nsec - start.tv_nsec) / 1000000;
        if (elapsed >= 5000)
            return -ETIMEDOUT;
        struct pollfd descriptor = {.fd = scoop_io_loop_descriptor(loop), .events = POLLIN};
        status = poll(&descriptor, 1, (int)(5000 - elapsed));
        if (status < 0 && errno != EINTR)
            return -errno;
    }
}
