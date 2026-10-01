#include "startup.h"
#include "thread.h"
#include <stdio.h>
#include <stdlib.h>

SCOOP_FIXTURE_IMAGES

int64_t m23_call_scalar(int64_t (*callback)(int64_t), int64_t value) {
    return callback(value);
}

struct NativeWide {
    int64_t first;
    int64_t second;
    int64_t third;
};
int64_t m23_call_wide(struct NativeWide (*callback)(struct NativeWide)) {
    struct NativeWide value = {17, -29, 54};
    struct NativeWide result = callback(value);
    return result.first + result.second + result.third;
}

int64_t m23_call_sink(void (*callback)(int64_t *)) {
    int64_t value = 0;
    callback(&value);
    return value;
}

int main(void) {
    int result = scoop_rt_run_program(
        fixture_images, sizeof fixture_images / sizeof *fixture_images, &fixture_root);
    bool collected = scoop_rt_thread_debug_gc_epoch() > 0;
    bool correct = result == 0 && (getenv("SCOOP_GC_STRESS_MOVE") == NULL || collected);
    if (!correct) {
        fprintf(stderr, "fixture status=%d, collected=%d\n", result, collected);
    }
    return correct ? 0 : 1;
}
