#include <stdint.h>

int64_t sum_values(const int32_t *values, int64_t count) {
    int64_t result = 0;
    for (int64_t index = 0; index < count; index++) {
        result += values[index];
    }
    return result;
}

int64_t sum_bytes(const uint8_t *bytes, int64_t count) {
    int64_t result = 0;
    for (int64_t index = 0; index < count; index++) {
        result += bytes[index];
    }
    return result;
}
