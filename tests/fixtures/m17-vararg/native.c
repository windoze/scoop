#include "../../../runtime/include/scoop_rt.h"

#include <stdint.h>

int64_t m17_native_sum(const ScoopArray *values) {
    const int64_t *elements = (const int64_t *)values->elements;
    int64_t result = 0;
    for (uint64_t index = 0; index < values->size; index++) {
        result += elements[index];
    }
    return result;
}

int64_t m17_native_add(int64_t value, int64_t delta) {
    return value + delta;
}
