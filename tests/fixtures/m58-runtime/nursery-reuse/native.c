#include "scoop_rt.h"

int64_t m58_minor_count(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.minor_collections;
}
