#include "scoop_rt.h"

int64_t full_collections(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.full_collections;
}
