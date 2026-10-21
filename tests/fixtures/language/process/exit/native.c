#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int32_t exit_code(void) {
    const char *value = getenv("M33_EXIT_CODE");
    return value == NULL ? 37 : (int32_t)strtol(value, NULL, 10);
}

static void unexpected_atexit(void) { fputs("atexit-bad\n", stderr); }

void register_exit_handler(void) { atexit(unexpected_atexit); }

_Noreturn void unexpected_release(void) { _exit(99); }
