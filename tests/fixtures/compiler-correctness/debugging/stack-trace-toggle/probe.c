#include "throwable.h"
#include <stdint.h>

/* Scoop ABI leaf: inspect the actual hidden slot, without allocating or polling. */
int32_t trace_has_snapshot(const ScoopThrowable *value) {
    return __atomic_load_n(&value->snapshot, __ATOMIC_ACQUIRE) != NULL;
}
