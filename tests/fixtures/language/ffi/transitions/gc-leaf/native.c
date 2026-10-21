#include "scoop_rt.h"
#include <assert.h>
#include <stdatomic.h>
#include <stdint.h>

typedef struct Pair {
    int32_t number;
    double fraction;
} Pair;

static _Atomic(int32_t) calls;

int32_t m33_observe(int32_t mode, int32_t value) {
    assert(scoop_rt_thread_debug_mode() == (uint32_t)mode);
    atomic_fetch_add_explicit(&calls, 1, memory_order_relaxed);
    return value;
}

Pair m33_pair(Pair value) {
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_DEBUG_MANAGED);
    value.number += 10;
    value.fraction += 0.5;
    return value;
}

int32_t m33_calls(void) { return atomic_load_explicit(&calls, memory_order_relaxed); }
