#ifndef SCOOP_PROFILING_GC_H
#define SCOOP_PROFILING_GC_H

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

typedef enum ScoopProfileGcPhase {
    SCOOP_PROFILE_GC_ROOTS,
    SCOOP_PROFILE_GC_REMEMBERED,
    SCOOP_PROFILE_GC_MARK,
    SCOOP_PROFILE_GC_PLAN,
    SCOOP_PROFILE_GC_COPY,
    SCOOP_PROFILE_GC_UPDATE,
    SCOOP_PROFILE_GC_RECLAIM,
    SCOOP_PROFILE_GC_VM_RETURN,
} ScoopProfileGcPhase;

typedef struct ScoopProfileGcCounters {
    uint64_t worker_cpu_ns;
    uint64_t root_slots;
    uint64_t mark_tasks;
    uint64_t traced_objects;
    uint64_t copied_bytes;
} ScoopProfileGcCounters;

#if defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING
void scoop_profile_gc_initialize(uint64_t origin_ns);
void scoop_profile_gc_stopping(uint64_t epoch);
void scoop_profile_gc_stopped(void);
void scoop_profile_gc_resumed(void);
void scoop_profile_gc_slice_start(ScoopProfileGcCounters counters);
void scoop_profile_gc_slice_finish(ScoopProfileGcCounters counters);
void scoop_profile_gc_cycle_begin(bool minor, bool incremental);
void scoop_profile_gc_cycle_end(bool completed);
void scoop_profile_gc_phase(ScoopProfileGcPhase phase, uint64_t begin, uint64_t end);
void scoop_profile_gc_write(FILE *output);
#else
#define scoop_profile_gc_initialize(origin_ns) ((void)0)
#define scoop_profile_gc_stopping(epoch) ((void)0)
#define scoop_profile_gc_stopped() ((void)0)
#define scoop_profile_gc_resumed() ((void)0)
#define scoop_profile_gc_slice_start(counters) ((void)0)
#define scoop_profile_gc_slice_finish(counters) ((void)0)
#define scoop_profile_gc_cycle_begin(minor, incremental) ((void)0)
#define scoop_profile_gc_cycle_end(completed) ((void)0)
#define scoop_profile_gc_phase(phase, begin, end) ((void)0)
#define scoop_profile_gc_write(output) ((void)0)
#endif

#endif
