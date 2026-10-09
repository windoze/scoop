#include "../m34-regions/native.c"

static ScoopGcMetrics before_mark;

void m34_parallel_begin(void) { scoop_rt_gc_debug_metrics(&before_mark); }

bool m34_parallel_check(void) {
    ScoopGcMetrics after;
    scoop_rt_gc_debug_metrics(&after);
    const char *setting = getenv("SCOOP_GC_WORKERS");
    unsigned long workers = setting == NULL ? 1 : strtoul(setting, NULL, 10);
    if (workers < 1 || workers > 8 || after.last_mark_workers != workers ||
        after.array_tasks <= before_mark.array_tasks || after.mark_ns <= before_mark.mark_ns ||
        after.worker_cpu_ns[workers - 1] <= before_mark.worker_cpu_ns[workers - 1]) {
        return false;
    }
    uint64_t marked = 0;
    for (size_t index = 0; index < 8; index++) {
        marked += after.worker_marked_objects[index] - before_mark.worker_marked_objects[index];
    }
    return marked == scoop_rt_gc_stats();
}
