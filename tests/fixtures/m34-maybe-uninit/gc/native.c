#include "scoop_rt.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

void m34_maybe_copy_gc(void *result, void *value, uint64_t reference_offset) {
    void **slot = (void **)((char *)value + reference_offset);
    void **slots[] = {slot};
    ScoopNativeRootFrame frame;
    scoop_rt_push_native_roots(&frame, slots, 1);
    scoop_runtime_gc_collect();
    memcpy(result, value, 16);
    scoop_rt_pop_native_roots(&frame);
}

uint8_t m34_maybe_is_zero(const void *value) {
    const unsigned char *bytes = value;
    for (size_t index = 0; index < 16; ++index) {
        if (bytes[index] != 0) {
            return 0;
        }
    }
    return 1;
}

int64_t m34_maybe_minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.minor_collections;
}

bool m34_maybe_moving(void) {
    const char *value = getenv("SCOOP_GC_STRESS_MOVE");
    return value != NULL && strcmp(value, "1") == 0;
}
