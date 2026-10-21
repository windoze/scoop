#include "scoop_rt.h"

static ScoopGcMetrics before;

void no_move_begin(void) { scoop_rt_gc_debug_metrics(&before); }

bool no_move_check(void) {
    ScoopGcMetrics after;
    scoop_rt_gc_debug_metrics(&after);
    return after.full_collections == before.full_collections + 1 &&
           scoop_rt_gc_debug_last_moved_count() == 0 && after.update_ns == before.update_ns;
}
