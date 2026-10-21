#include <stdatomic.h>
#include <stdint.h>

/* Test-only observation at the real GC operation boundary. */
static _Atomic int64_t positive;
static _Atomic int64_t negative;
static _Atomic int notifications;

void scoop_gc_adjust_pressure(int64_t delta) {
    atomic_fetch_add_explicit(&notifications, 1, memory_order_relaxed);
    if (delta > 0)
        atomic_fetch_add_explicit(&positive, delta, memory_order_relaxed);
    else
        atomic_fetch_add_explicit(&negative, delta, memory_order_relaxed);
}

int64_t m45_positive(void) { return atomic_load_explicit(&positive, memory_order_relaxed); }
int64_t m45_negative(void) { return atomic_load_explicit(&negative, memory_order_relaxed); }
int m45_notifications(void) { return atomic_load_explicit(&notifications, memory_order_relaxed); }
