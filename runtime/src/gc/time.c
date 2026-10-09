#define _POSIX_C_SOURCE 200809L
#include <time.h>

#include "heap_internal.h"

static uint64_t clock_ns(clockid_t clock) {
    struct timespec value;
    if (clock_gettime(clock, &value) != 0) {
        heap_fatal("cannot measure GC time");
    }
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

uint64_t scoop_gc_monotonic_ns(void) { return clock_ns(CLOCK_MONOTONIC); }

uint64_t scoop_gc_thread_cpu_ns(void) { return clock_ns(CLOCK_THREAD_CPUTIME_ID); }
