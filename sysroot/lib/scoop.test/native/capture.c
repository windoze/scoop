#define _POSIX_C_SOURCE 200809L
#include "test_io.h"

#include <inttypes.h>
#include <limits.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

static int saved_stdout = -1;
static int saved_stderr = -1;
static struct timespec started;

static int flush_streams(void) {
    int output = fflush(stdout);
    int error = fflush(stderr);
    return output || error ? -1 : 0;
}

static int restore(void) {
    int result = 0;
    if (saved_stdout >= 0) {
        if (dup2(saved_stdout, STDOUT_FILENO) < 0) result = -1;
        if (close(saved_stdout) < 0) result = -1;
        saved_stdout = -1;
    }
    if (saved_stderr >= 0) {
        if (dup2(saved_stderr, STDERR_FILENO) < 0) result = -1;
        if (close(saved_stderr) < 0) result = -1;
        saved_stderr = -1;
    }
    return result;
}

int scoop_test_capture_begin(uint64_t sequence) {
    if (!getenv("SCOOP_TEST_NOCAPTURE")) {
        if (flush_streams() < 0) return -1;
        saved_stdout = dup(STDOUT_FILENO);
        saved_stderr = dup(STDERR_FILENO);
        char name[64];
        snprintf(name, sizeof(name), "%" PRIu64 ".stdout", sequence);
        FILE *output = scoop_test_open_output(name);
        snprintf(name, sizeof(name), "%" PRIu64 ".stderr", sequence);
        FILE *error = scoop_test_open_output(name);
        int failed = saved_stdout < 0 || saved_stderr < 0 || !output || !error;
        if (!failed && (dup2(fileno(output), STDOUT_FILENO) < 0 ||
                        dup2(fileno(error), STDERR_FILENO) < 0)) failed = 1;
        if (output && fclose(output)) failed = 1;
        if (error && fclose(error)) failed = 1;
        if (failed) {
            restore();
            return -1;
        }
    }
    if (clock_gettime(CLOCK_MONOTONIC, &started)) {
        restore();
        return -1;
    }
    return 0;
}

int64_t scoop_test_capture_end(void) {
    struct timespec ended;
    int failed = clock_gettime(CLOCK_MONOTONIC, &ended);
    if (saved_stdout >= 0 || saved_stderr >= 0) {
        if (flush_streams() < 0) failed = 1;
        if (restore() < 0) failed = 1;
    }
    if (failed || ended.tv_sec < started.tv_sec) return -1;
    uint64_t seconds = (uint64_t)(ended.tv_sec - started.tv_sec);
    int64_t nanos = ended.tv_nsec - started.tv_nsec;
    if (nanos < 0) {
        if (seconds == 0) return -1;
        --seconds;
        nanos += 1000000000;
    }
    if (seconds > ((uint64_t)INT64_MAX - (uint64_t)nanos) / 1000000000) return -1;
    return (int64_t)(seconds * 1000000000 + (uint64_t)nanos);
}
