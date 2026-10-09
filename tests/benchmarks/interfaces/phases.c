#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static uint64_t wall_start;
static uint64_t cpu_start;

static uint64_t now(clockid_t clock) {
    struct timespec value;
    if (clock_gettime(clock, &value) != 0) {
        abort();
    }
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

int32_t bench_kind(void) {
    const char *value = getenv("SCOOP_BENCH_KIND");
    return value == NULL ? 0 : (int32_t)strtol(value, NULL, 10);
}

int32_t bench_count(void) {
    const char *value = getenv("SCOOP_BENCH_COUNT");
    return value == NULL ? 2000000 : (int32_t)strtol(value, NULL, 10);
}

void bench_begin(void) {
    wall_start = now(CLOCK_MONOTONIC);
    cpu_start = now(CLOCK_PROCESS_CPUTIME_ID);
}

void bench_end(void) {
    uint64_t wall = now(CLOCK_MONOTONIC) - wall_start;
    uint64_t cpu = now(CLOCK_PROCESS_CPUTIME_ID) - cpu_start;
    fprintf(stderr, "{\"wall_ns\":%llu,\"cpu_ns\":%llu}\n", (unsigned long long)wall,
            (unsigned long long)cpu);
}
