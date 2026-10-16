#define _GNU_SOURCE 1
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <time.h>

struct ScoopPlatformTime {
    int64_t seconds, nanoseconds;
};

struct ScoopPlatformTime scoop_platform_clock(bool monotonic) {
    struct timespec value;
    if (clock_gettime(monotonic ? CLOCK_MONOTONIC : CLOCK_REALTIME, &value) < 0)
        return (struct ScoopPlatformTime){0, -1};
    return (struct ScoopPlatformTime){value.tv_sec, value.tv_nsec};
}

int scoop_platform_sleep(int64_t seconds, int32_t nanoseconds) {
    if (seconds < 0 || nanoseconds < 0 || nanoseconds >= 1000000000) {
        errno = EINVAL;
        return -1;
    }
    if (seconds == 0 && nanoseconds == 0)
        return 0;
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) < 0)
        return -1;
    int64_t nanos = now.tv_nsec + (int64_t)nanoseconds;
    int64_t carry = nanos / 1000000000;
    if (seconds > INT64_MAX - now.tv_sec - carry) {
        errno = EOVERFLOW;
        return -1;
    }
    int64_t deadline_seconds = now.tv_sec + seconds + carry;
    int64_t deadline_nanos = nanos % 1000000000;
    for (;;) {
        struct timespec remaining;
        remaining.tv_sec = (time_t)(deadline_seconds - now.tv_sec);
        remaining.tv_nsec = (long)(deadline_nanos - now.tv_nsec);
        if (remaining.tv_nsec < 0) {
            --remaining.tv_sec;
            remaining.tv_nsec += 1000000000;
        }
        if (remaining.tv_sec < 0 || (remaining.tv_sec == 0 && remaining.tv_nsec == 0))
            return 0;
        if (nanosleep(&remaining, NULL) == 0)
            return 0;
        if (errno != EINTR)
            return -1;
        if (clock_gettime(CLOCK_MONOTONIC, &now) < 0)
            return -1;
    }
}
