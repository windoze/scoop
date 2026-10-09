#include "scoop_rt.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static ScoopGcMetrics before;

int32_t m34_mark_kind(void) {
    const char *kind = getenv("SCOOP_BENCH_GRAPH");
    return kind != NULL && strcmp(kind, "array") == 0   ? 1
           : kind != NULL && strcmp(kind, "chain") == 0 ? 2
                                                        : 0;
}

int64_t m34_mark_count(void) {
    const char *count = getenv("SCOOP_BENCH_NODES");
    return count == NULL ? 131071 : strtoll(count, NULL, 10);
}

int32_t m34_mark_rounds(void) {
    const char *rounds = getenv("SCOOP_BENCH_ROUNDS");
    return rounds == NULL ? 5 : (int32_t)strtol(rounds, NULL, 10);
}

void m34_mark_begin(void) { scoop_rt_gc_debug_metrics(&before); }

void m34_mark_end(int32_t round) {
    ScoopGcMetrics after;
    scoop_rt_gc_debug_metrics(&after);
    fprintf(stderr,
            "{\"scoop_mark_phase\":1,\"round\":%" PRId32 ",\"pause_ns\":%" PRIu64
            ",\"traced_objects\":%" PRIu64 ",\"live_objects\":%" PRIu64 ",\"mapped_bytes\":%" PRIu64
            ",\"current_rss_bytes\":%" PRIu64 ",\"peak_rss_bytes\":%" PRIu64,
            round, after.pause_ns - before.pause_ns, after.traced_objects - before.traced_objects,
            scoop_rt_gc_stats(), after.mapped_bytes, after.current_rss_bytes, after.peak_rss_bytes);
#ifdef SCOOP_BENCH_PARALLEL_METRICS
    fprintf(stderr,
            ",\"workers\":%" PRIu64 ",\"stop_wait_ns\":%" PRIu64 ",\"root_scan_ns\":%" PRIu64
            ",\"remembered_scan_ns\":%" PRIu64 ",\"mark_ns\":%" PRIu64 ",\"plan_ns\":%" PRIu64
            ",\"copy_ns\":%" PRIu64 ",\"update_ns\":%" PRIu64 ",\"reclaim_ns\":%" PRIu64
            ",\"vm_return_ns\":%" PRIu64 ",\"mark_reference_slots\":%" PRIu64
            ",\"array_tasks\":%" PRIu64 ",\"stolen_tasks\":%" PRIu64,
            after.last_mark_workers, after.stop_wait_ns - before.stop_wait_ns,
            after.root_scan_ns - before.root_scan_ns,
            after.remembered_scan_ns - before.remembered_scan_ns, after.mark_ns - before.mark_ns,
            after.plan_ns - before.plan_ns, after.copy_ns - before.copy_ns,
            after.update_ns - before.update_ns, after.reclaim_ns - before.reclaim_ns,
            after.vm_return_ns - before.vm_return_ns,
            after.mark_reference_slots - before.mark_reference_slots,
            after.array_tasks - before.array_tasks, after.stolen_tasks - before.stolen_tasks);
    fputs(",\"worker_cpu_ns\":[", stderr);
    for (size_t index = 0; index < 8; index++) {
        fprintf(stderr, "%s%" PRIu64, index == 0 ? "" : ",",
                after.worker_cpu_ns[index] - before.worker_cpu_ns[index]);
    }
    fputs("],\"worker_marked_objects\":[", stderr);
    for (size_t index = 0; index < 8; index++) {
        fprintf(stderr, "%s%" PRIu64, index == 0 ? "" : ",",
                after.worker_marked_objects[index] - before.worker_marked_objects[index]);
    }
    fputc(']', stderr);
#endif
    fputs("}\n", stderr);
}
