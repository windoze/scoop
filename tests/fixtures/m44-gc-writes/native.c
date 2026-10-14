#include "scoop_rt.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

int64_t m44_minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.minor_collections;
}

bool m44_moving(void) {
    const char *value = getenv("SCOOP_GC_STRESS_MOVE");
    return value != NULL && strcmp(value, "1") == 0;
}
