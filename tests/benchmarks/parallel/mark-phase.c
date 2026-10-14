#include "scoop_rt.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef SCOOP_BENCH_WORLD_TIMING
#include <assert.h>
#include <time.h>
#define SCOOP_THREAD_TESTING
#include "../../../runtime/src/thread/testing.h"

static uint64_t boundaries[3];
static unsigned boundary_count;

void scoop_thread_test_point(ScoopThreadTestPoint point) {
    unsigned index;
    switch (point) {
    case SCOOP_TEST_COLLECTOR_STOPPING:
        index = 0;
        break;
    case SCOOP_TEST_COLLECTOR_STOPPED:
        index = 1;
        break;
    case SCOOP_TEST_COLLECTOR_RUNNING:
        index = 2;
        break;
    default:
        return;
    }
    struct timespec now;
    assert(clock_gettime(CLOCK_MONOTONIC, &now) == 0);
    boundaries[index] = (uint64_t)now.tv_sec * UINT64_C(1000000000) + (uint64_t)now.tv_nsec;
    boundary_count++;
}
#endif

static ScoopGcMetrics before;

int32_t m34_mark_kind(void) {
    const char *kind = getenv("SCOOP_BENCH_GRAPH");
    return kind != NULL && strcmp(kind, "array") == 0     ? 1
           : kind != NULL && strcmp(kind, "chain") == 0   ? 2
           : kind != NULL && strcmp(kind, "compact") == 0 ? 3
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

void m34_mark_begin(void) {
    scoop_rt_gc_debug_metrics(&before);
#ifdef SCOOP_BENCH_WORLD_TIMING
    boundary_count = 0;
#endif
}

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
    fprintf(stderr, ",\"moved_objects\":%" PRIu64, scoop_rt_gc_debug_last_moved_count());
#ifdef SCOOP_BENCH_PARALLEL_METRICS
    fprintf(stderr,
            ",\"source_regions\":%" PRIu64 ",\"target_regions\":%" PRIu64
            ",\"pin_blocked_regions\":%" PRIu64,
            after.last_full_source_regions, after.last_full_target_regions,
            after.last_full_pin_blocked_regions);
#endif
#ifdef SCOOP_BENCH_WORLD_TIMING
    assert(boundary_count == 3);
    assert(boundaries[0] <= boundaries[1] && boundaries[1] <= boundaries[2]);
    fprintf(stderr,
            ",\"stopping_ns\":%" PRIu64 ",\"collecting_ns\":%" PRIu64 ",\"running_ns\":%" PRIu64
            ",\"quiescence_ns\":%" PRIu64 ",\"world_stopped_ns\":%" PRIu64
            ",\"request_to_running_ns\":%" PRIu64,
            boundaries[0], boundaries[1], boundaries[2], boundaries[1] - boundaries[0],
            boundaries[2] - boundaries[1], boundaries[2] - boundaries[0]);
#endif
    fputs("}\n", stderr);
}
