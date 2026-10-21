#include "scoop_rt.h"

void m58_dirty_longs(int64_t *values, int64_t count) {
    scoop_rt_gc_write_barrier(values, (size_t)count * sizeof *values);
}

int64_t m58_reference_slots(void) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    return (int64_t)metrics.old_reference_slots;
}
