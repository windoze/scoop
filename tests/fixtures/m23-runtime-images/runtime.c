#include "startup.h"
#include <inttypes.h>
#include <stdio.h>

SCOOP_FIXTURE_IMAGES

void m23_trace(int64_t value) { printf("%" PRId64 "\n", value); }

int main(void) {
    setvbuf(stdout, NULL, _IONBF, 0);
    return scoop_rt_run_program(
        fixture_images, sizeof fixture_images / sizeof *fixture_images, &fixture_root);
}
