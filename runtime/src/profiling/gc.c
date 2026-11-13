#include "gc.h"

#if defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING

#include <inttypes.h>
#include <stdatomic.h>

#include "../platform/primitives.h"

enum { EVENT_CAPACITY = 8192 };
typedef enum EventKind { CYCLE_BEGIN, CYCLE_END, SLICE, PHASE } EventKind;
typedef struct Event {
    EventKind kind;
    uint64_t cycle;
    uint64_t slice;
    uint64_t begin;
    uint64_t end;
    union {
        struct {
            bool minor;
            bool incremental;
            bool completed;
        } cycle;
        struct {
            uint64_t stopped;
            ScoopProfileGcCounters counters;
        } slice;
        ScoopProfileGcPhase phase;
    } detail;
} Event;

/* The existing world-phase ownership serializes producers, including the
 * RUNNING publication. Finished entries are never modified or reused. Exit
 * reads one released prefix without waiting for other application threads. */
static Event events[EVENT_CAPACITY];
static size_t used;
static _Atomic(size_t) published;
static _Atomic(uint64_t) dropped;
static _Atomic(uint64_t) clock_failures;
static _Atomic(bool) initialized;
static uint64_t origin;
static uint64_t next_cycle;
static uint64_t current_cycle;
static Event slice;
static ScoopProfileGcCounters before;

static bool enabled(void) { return atomic_load_explicit(&initialized, memory_order_acquire); }

static uint64_t now(void) {
    uint64_t result;
    if (scoop_runtime_platform_monotonic_ns(&result))
        return result;
    atomic_fetch_add_explicit(&clock_failures, 1, memory_order_relaxed);
    return 0;
}

static void append(Event event) {
    if (used == EVENT_CAPACITY) {
        atomic_fetch_add_explicit(&dropped, 1, memory_order_relaxed);
        return;
    }
    events[used++] = event;
    atomic_store_explicit(&published, used, memory_order_release);
}

void scoop_profile_gc_initialize(uint64_t origin_ns) {
    origin = origin_ns;
    atomic_store_explicit(&initialized, true, memory_order_release);
}

void scoop_profile_gc_stopping(uint64_t epoch) {
    if (!enabled())
        return;
    slice = (Event){.kind = SLICE, .slice = epoch, .begin = now()};
}

void scoop_profile_gc_stopped(void) {
    if (enabled())
        slice.detail.slice.stopped = now();
}

void scoop_profile_gc_resumed(void) {
    if (!enabled())
        return;
    slice.end = now();
    append(slice);
}

void scoop_profile_gc_slice_start(ScoopProfileGcCounters counters) {
    if (enabled())
        before = counters;
}

void scoop_profile_gc_slice_finish(ScoopProfileGcCounters counters) {
    if (!enabled())
        return;
    slice.detail.slice.counters = (ScoopProfileGcCounters){
        .worker_cpu_ns = counters.worker_cpu_ns - before.worker_cpu_ns,
        .root_slots = counters.root_slots - before.root_slots,
        .mark_tasks = counters.mark_tasks - before.mark_tasks,
        .traced_objects = counters.traced_objects - before.traced_objects,
        .copied_bytes = counters.copied_bytes - before.copied_bytes,
    };
}

void scoop_profile_gc_cycle_begin(bool minor, bool incremental) {
    if (!enabled())
        return;
    current_cycle = ++next_cycle;
    Event event = {
        .kind = CYCLE_BEGIN, .cycle = current_cycle, .slice = slice.slice, .begin = now()};
    event.detail.cycle.minor = minor;
    event.detail.cycle.incremental = incremental;
    append(event);
}

void scoop_profile_gc_cycle_end(bool completed) {
    if (!enabled())
        return;
    Event event = {.kind = CYCLE_END, .cycle = current_cycle, .slice = slice.slice, .end = now()};
    event.detail.cycle.completed = completed;
    append(event);
    current_cycle = 0;
}

void scoop_profile_gc_phase(ScoopProfileGcPhase phase, uint64_t begin, uint64_t end) {
    if (!enabled())
        return;
    Event event = {
        .kind = PHASE, .cycle = current_cycle, .slice = slice.slice, .begin = begin, .end = end};
    event.detail.phase = phase;
    append(event);
}

static void time_field(FILE *output, const char *name, uint64_t time) {
    fprintf(output, ",\"%s\":", name);
    if (time != 0 && time >= origin)
        fprintf(output, "%" PRIu64, time - origin);
    else
        fputs("null", output);
}

static void elapsed_field(FILE *output, const char *name, uint64_t begin, uint64_t end) {
    fprintf(output, ",\"%s\":", name);
    if (begin != 0 && begin >= origin && end >= begin)
        fprintf(output, "%" PRIu64, end - begin);
    else
        fputs("null", output);
}

void scoop_profile_gc_write(FILE *output) {
    size_t count = atomic_load_explicit(&published, memory_order_acquire);
    static const char *const phases[] = {"roots", "remembered", "mark",    "plan",
                                         "copy",  "update",     "reclaim", "vm-return"};
    for (size_t index = 0; index < count; index++) {
        const Event *event = &events[index];
        fputs("{\"scoop_profile\":1,\"event\":", output);
        switch (event->kind) {
        case CYCLE_BEGIN:
            fprintf(output,
                    "\"gc-cycle-begin\",\"cycle\":%" PRIu64 ",\"slice\":%" PRIu64
                    ",\"collection\":\"%s\",\"incremental\":%s",
                    event->cycle, event->slice, event->detail.cycle.minor ? "minor" : "full",
                    event->detail.cycle.incremental ? "true" : "false");
            time_field(output, "at_ns", event->begin);
            break;
        case CYCLE_END:
            fprintf(output,
                    "\"gc-cycle-end\",\"cycle\":%" PRIu64 ",\"slice\":%" PRIu64
                    ",\"status\":\"%s\"",
                    event->cycle, event->slice,
                    event->detail.cycle.completed ? "completed" : "restarted");
            time_field(output, "at_ns", event->end);
            break;
        case SLICE: {
            const ScoopProfileGcCounters *counters = &event->detail.slice.counters;
            fprintf(output, "\"gc-slice\",\"slice\":%" PRIu64, event->slice);
            time_field(output, "requested_ns", event->begin);
            time_field(output, "stopped_ns", event->detail.slice.stopped);
            time_field(output, "resumed_ns", event->end);
            elapsed_field(output, "stop_wait_ns", event->begin, event->detail.slice.stopped);
            elapsed_field(output, "pause_ns", event->detail.slice.stopped, event->end);
            fprintf(output,
                    ",\"worker_cpu_ns\":%" PRIu64 ",\"root_slots\":%" PRIu64
                    ",\"mark_tasks\":%" PRIu64 ",\"traced_objects\":%" PRIu64
                    ",\"copied_bytes\":%" PRIu64,
                    counters->worker_cpu_ns, counters->root_slots, counters->mark_tasks,
                    counters->traced_objects, counters->copied_bytes);
            break;
        }
        case PHASE:
            fprintf(output,
                    "\"gc-phase\",\"cycle\":%" PRIu64 ",\"slice\":%" PRIu64 ",\"phase\":\"%s\"",
                    event->cycle, event->slice, phases[event->detail.phase]);
            time_field(output, "begin_ns", event->begin);
            time_field(output, "end_ns", event->end);
            break;
        }
        fputs("}\n", output);
    }
    fprintf(output,
            "{\"scoop_profile\":1,\"event\":\"gc-trace\",\"capacity\":%d,\"events\":%zu,"
            "\"dropped\":%" PRIu64 ",\"clock_failures\":%" PRIu64 "}\n",
            EVENT_CAPACITY, count, atomic_load_explicit(&dropped, memory_order_relaxed),
            atomic_load_explicit(&clock_failures, memory_order_relaxed));
}

#endif
