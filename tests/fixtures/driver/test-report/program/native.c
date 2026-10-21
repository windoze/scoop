#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

int32_t report_mode(void) {
    const char *mode = getenv("M43_REPORT_MODE");
    return mode ? atoi(mode) : 0;
}

static void finish(void) {
    fputs("shutdown\n", stdout);
    fflush(stdout);
    if (report_mode() == 3) _Exit(19);
}

int32_t report_prepare(void) {
    fputs("startup\n", stdout);
    fflush(stdout);
    if (atexit(finish)) abort();
    return report_mode();
}

void report_body(void) {
    fputs("native\n", stdout);
    if (report_mode() != 6) fputs("{\"event\":\"complete\"}\n", stderr);
    fflush(stdout);
    fflush(stderr);
    switch (report_mode()) {
        case 1: exit(0);
        case 2: abort();
        case 5: fputc(255, stdout); fputc(254, stderr); break;
        default: break;
    }
}
