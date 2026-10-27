#include "heap_internal.h"

uint64_t scoop_gc_monotonic_ns(void) {
    uint64_t value;
    if (!scoop_platform_monotonic_ns(&value)) {
        heap_fatal("cannot measure GC time");
    }
    return value;
}

uint64_t scoop_gc_thread_cpu_ns(void) {
    uint64_t value;
    if (!scoop_platform_thread_cpu_ns(&value)) {
        heap_fatal("cannot measure GC thread CPU time");
    }
    return value;
}
