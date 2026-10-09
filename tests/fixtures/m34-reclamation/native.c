#include "../m34-regions/native.c"

uint64_t m34_reclamation_unmapped(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return metrics.unmapped_bytes;
}

uint64_t m34_reclamation_discards(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return metrics.discard_calls;
}
