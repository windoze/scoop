#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#define WIN32_LEAN_AND_MEAN
#else
#define _POSIX_C_SOURCE 200809L
#endif
#include "test_io.h"

#include <inttypes.h>
#include <limits.h>
#include <stdlib.h>
#ifdef _WIN32
#include <io.h>
#include <windows.h>
#define dup _dup
#define dup2 _dup2
#define close _close
#define fileno _fileno
#define STDOUT_FILENO 1
#define STDERR_FILENO 2
#else
#include <time.h>
#include <unistd.h>
#endif

static int saved_stdout = -1;
static int saved_stderr = -1;
static int64_t started;

static int64_t monotonic_ns(void) {
#ifdef _WIN32
    LARGE_INTEGER counter, frequency;
    if (!QueryPerformanceCounter(&counter) || !QueryPerformanceFrequency(&frequency) ||
        counter.QuadPart < 0 || frequency.QuadPart <= 0)
        return -1;
    uint64_t ticks = (uint64_t)counter.QuadPart, hz = (uint64_t)frequency.QuadPart;
    uint64_t seconds = ticks / hz;
    uint64_t nanos = (ticks % hz) * UINT64_C(1000000000) / hz;
#else
    struct timespec counter;
    if (clock_gettime(CLOCK_MONOTONIC, &counter) || counter.tv_sec < 0)
        return -1;
    uint64_t seconds = (uint64_t)counter.tv_sec;
    uint64_t nanos = (uint64_t)counter.tv_nsec;
#endif
    if (seconds > ((uint64_t)INT64_MAX - nanos) / 1000000000)
        return -1;
    return (int64_t)(seconds * 1000000000 + nanos);
}

static int flush_streams(void) {
    int output = fflush(stdout);
    int error = fflush(stderr);
    return output || error ? -1 : 0;
}

static int restore(void) {
    int result = 0;
    if (saved_stdout >= 0) {
        if (dup2(saved_stdout, STDOUT_FILENO) < 0)
            result = -1;
        if (close(saved_stdout) < 0)
            result = -1;
        saved_stdout = -1;
    }
    if (saved_stderr >= 0) {
        if (dup2(saved_stderr, STDERR_FILENO) < 0)
            result = -1;
        if (close(saved_stderr) < 0)
            result = -1;
        saved_stderr = -1;
    }
    return result;
}

int scoop_test_capture_begin(uint64_t sequence) {
    if (!getenv("SCOOP_TEST_NOCAPTURE")) {
        if (flush_streams() < 0)
            return -1;
        saved_stdout = dup(STDOUT_FILENO);
        saved_stderr = dup(STDERR_FILENO);
        char name[64];
        snprintf(name, sizeof(name), "%" PRIu64 ".stdout", sequence);
        FILE *output = scoop_test_open_output(name);
        snprintf(name, sizeof(name), "%" PRIu64 ".stderr", sequence);
        FILE *error = scoop_test_open_output(name);
        int failed = saved_stdout < 0 || saved_stderr < 0 || !output || !error;
        if (!failed &&
            (dup2(fileno(output), STDOUT_FILENO) < 0 || dup2(fileno(error), STDERR_FILENO) < 0))
            failed = 1;
        if (output && fclose(output))
            failed = 1;
        if (error && fclose(error))
            failed = 1;
        if (failed) {
            restore();
            return -1;
        }
    }
    started = monotonic_ns();
    if (started < 0) {
        restore();
        return -1;
    }
    return 0;
}

int64_t scoop_test_capture_end(void) {
    int64_t ended = monotonic_ns();
    int failed = ended < started;
    if (saved_stdout >= 0 || saved_stderr >= 0) {
        if (flush_streams() < 0)
            failed = 1;
        if (restore() < 0)
            failed = 1;
    }
    return failed ? -1 : ended - started;
}
