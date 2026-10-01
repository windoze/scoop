#include "startup.h"
#include "thread.h"
#include <stdio.h>
#include <stdlib.h>

SCOOP_FIXTURE_IMAGES

#ifndef SCOOP_FIXTURE_METADATA_CHECK
#define SCOOP_FIXTURE_METADATA_CHECK() true
#endif

int main(void) {
    int result = scoop_rt_run_program(
        fixture_images, sizeof fixture_images / sizeof *fixture_images, &fixture_root);
    bool collected = scoop_rt_thread_debug_gc_epoch() > 0;
    bool correct = result == 0 && (getenv("SCOOP_GC_STRESS_MOVE") == NULL || collected);
    correct = correct && SCOOP_FIXTURE_METADATA_CHECK();
    if (!correct) {
        fprintf(stderr, "fixture status=%d, collected=%d\n", result, collected);
    }
    return correct ? 0 : 1;
}
