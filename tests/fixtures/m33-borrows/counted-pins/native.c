#include <stdatomic.h>
#include <stdint.h>

static _Atomic(int32_t) released;

void counted_pin_release(int32_t value) {
    atomic_fetch_add_explicit(&released, value, memory_order_relaxed);
}

int32_t counted_pin_released(void) { return atomic_load_explicit(&released, memory_order_relaxed); }
