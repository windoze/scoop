#define _DARWIN_C_SOURCE 1
#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <sys/resource.h>
#include <time.h>
int64_t scoop_measure_clock(int kind) {
    if (kind == 2) {
        struct rusage usage;
        getrusage(RUSAGE_SELF, &usage);
        return usage.ru_nvcsw;
    }
    struct timespec time;
    clock_gettime(kind ? CLOCK_PROCESS_CPUTIME_ID : CLOCK_MONOTONIC, &time);
    return (int64_t)time.tv_sec * 1000000000 + time.tv_nsec;
}
