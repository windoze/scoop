#include "scoop_rt.h"

#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

static bool enabled(const char *name) {
    const char *value = getenv(name);
    return value != NULL && strcmp(value, "1") == 0;
}

bool m34_region_stress(void) {
    return enabled("SCOOP_GC_STRESS_MOVE") || enabled("SCOOP_GC_STRESS_MINOR");
}

bool m34_region_moving(void) { return enabled("SCOOP_GC_STRESS_MOVE"); }

uint64_t m34_region_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return metrics.region_count;
}

uint64_t m34_region_minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return metrics.minor_collections;
}
