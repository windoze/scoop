#define _POSIX_C_SOURCE 200809L
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int32_t process_mode(void) {
    const char *mode = getenv("M43_PROCESS_MODE");
    return mode ? atoi(mode) : 0;
}

void process_body(void) {
    if (process_mode() == 2) {
        raise(SIGSEGV);
        abort();
    }
    const char *ready = getenv("M43_PROCESS_READY");
    if (!ready) abort();
    FILE *file = fopen(ready, "wb");
    if (!file || fputs("ready\n", file) == EOF || fclose(file)) abort();
    for (;;) pause();
}
