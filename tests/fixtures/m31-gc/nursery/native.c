#include "scoop_rt.h"

static int64_t released;

void release_value(int64_t value) { released += value; }

int64_t release_count(void) { return released; }

int64_t minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.minor_collections;
}
