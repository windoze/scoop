#include "../../../runtime/include/scoop_rt.h"

#include <stdint.h>

int32_t m17_native_sum(const ScoopArray *values) {
    const int32_t *elements = (const int32_t *)values->elements;
    int32_t result = 0;
    for (uint64_t index = 0; index < values->size; index++) {
        result += elements[index];
    }
    return result;
}

int32_t m17_native_add(int32_t value, int32_t delta) {
    return value + delta;
}
