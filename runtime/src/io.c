#include <stdio.h>

#include "scoop_rt.h"

void scoop_rt_stdout_write(const uint8_t *bytes, int64_t length) {
    fwrite(bytes, 1, (size_t)length, stdout);
}

void scoop_rt_stderr_write(const uint8_t *bytes, int64_t length) {
    fwrite(bytes, 1, (size_t)length, stderr);
}

void scoop_rt_flush_stdout(void) { fflush(stdout); }
