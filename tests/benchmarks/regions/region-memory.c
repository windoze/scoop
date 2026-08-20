/* Phase samples also work with the M34-6 runtime, which has no RSS counters. */
#include "scoop_rt.h"

#include <inttypes.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>
#ifdef __APPLE__
#include <mach/mach.h>
#endif

static uint64_t current_rss(void) {
#ifdef __APPLE__
    mach_task_basic_info_data_t info;
    mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
    return task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&info, &count) ==
                   KERN_SUCCESS
               ? info.resident_size
               : 0;
#else
    uint64_t bytes = 0;
    FILE *statm = fopen("/proc/self/statm", "r");
    if (statm != NULL) {
        unsigned long virtual_pages, resident_pages;
        if (fscanf(statm, "%lu %lu", &virtual_pages, &resident_pages) == 2) {
            bytes = (uint64_t)resident_pages * (uint64_t)sysconf(_SC_PAGESIZE);
        }
        fclose(statm);
    }
    return bytes;
#endif
}

bool m34_memory_pins(void) {
    const char *value = getenv("SCOOP_BENCH_PIN");
    return value != NULL && strcmp(value, "1") == 0;
}

void m34_memory_sample(int32_t phase) {
    ScoopGcMetrics metrics;
    scoop_rt_gc_debug_metrics(&metrics);
    struct rusage usage;
    uint64_t peak = getrusage(RUSAGE_SELF, &usage) == 0 ? (uint64_t)usage.ru_maxrss : 0;
#ifndef __APPLE__
    peak *= 1024;
#endif
    fprintf(stderr,
            "{\"memory_phase\":%" PRId32 ",\"region_count\":%" PRIu64 ",\"mapped_bytes\":%" PRIu64
            ",\"current_rss_bytes\":%" PRIu64 ",\"peak_rss_bytes\":%" PRIu64 "}\n",
            phase, metrics.region_count, metrics.mapped_bytes, current_rss(), peak);
}
