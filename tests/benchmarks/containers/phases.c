#define _POSIX_C_SOURCE 200809L
#include "scoop_rt.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

static ScoopGcMetrics before;
static uint64_t wall_start;
static uint64_t cpu_start;

static uint64_t now(clockid_t clock) {
    struct timespec value;
    if (clock_gettime(clock, &value) != 0) {
        abort();
    }
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

int32_t m34_container_kind(void) {
    const char *value = getenv("SCOOP_BENCH_KIND");
    return value == NULL ? 0 : (int32_t)strtol(value, NULL, 10);
}

int64_t m34_container_count(void) {
    const char *value = getenv("SCOOP_BENCH_COUNT");
    return value == NULL ? 262144 : strtoll(value, NULL, 10);
}

void m34_container_begin(void) {
    scoop_rt_gc_debug_metrics(&before);
    cpu_start = now(CLOCK_PROCESS_CPUTIME_ID);
    wall_start = now(CLOCK_MONOTONIC);
}

void m34_container_end(int32_t phase) {
    uint64_t wall = now(CLOCK_MONOTONIC) - wall_start;
    uint64_t cpu = now(CLOCK_PROCESS_CPUTIME_ID) - cpu_start;
    ScoopGcMetrics after;
    scoop_rt_gc_debug_metrics(&after);
    fprintf(stderr,
            "{\"scoop_container_phase\":1,\"phase\":%" PRId32 ",\"wall_ns\":%" PRIu64
            ",\"cpu_ns\":%" PRIu64 ",\"allocated_bytes\":%" PRIu64 ",\"pause_ns\":%" PRIu64
            ",\"mark_ns\":%" PRIu64 ",\"mark_reference_slots\":%" PRIu64
            ",\"mapped_bytes\":%" PRIu64 ",\"current_rss_bytes\":%" PRIu64
            ",\"peak_rss_bytes\":%" PRIu64 "}\n",
            phase, wall, cpu, after.allocated_bytes - before.allocated_bytes,
            after.pause_ns - before.pause_ns, after.mark_ns - before.mark_ns,
            after.mark_reference_slots - before.mark_reference_slots, after.mapped_bytes,
            after.current_rss_bytes, after.peak_rss_bytes);
}

void m34_container_layout(const void *object) {
    typedef struct {
        ScoopObjectHeader header;
        const ScoopArray *storage;
        int64_t count;
    } List;
    const List *list = object;
    const ScoopArray *storage = list->storage;
    const ScoopTypeInstanceShapeV1 *shape = &storage->header.td->instance_shape;
    fprintf(stderr,
            "{\"scoop_container_layout\":1,\"capacity\":%" PRIu64 ",\"count\":%" PRId64
            ",\"stride\":%" PRIu64 ",\"alignment\":%" PRIu64 ",\"storage_bytes\":%" PRIu64 "}\n",
            storage->size, list->count, shape->inline_stride, shape->inline_alignment,
            scoop_rt_gc_debug_allocation_size(storage));
}
