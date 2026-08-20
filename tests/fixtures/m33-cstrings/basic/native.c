#include <stdint.h>
#include <string.h>

int64_t cstring_sum(const int8_t *value, int64_t expected_length) {
    size_t length = strlen((const char *)value);
    if ((uint64_t)expected_length != length) {
        return -1;
    }
    int64_t result = 0;
    for (size_t index = 0; index < length; ++index) {
        result += (uint8_t)value[index];
    }
    return result;
}

const int8_t *native_cstring(int32_t kind) {
    static const unsigned char suffix[] = {'A', 0, 0xff};
    static const unsigned char invalid[] = {'A', 0xe1, 0x80, 0};
    static const unsigned char empty[] = {0};
    return (const int8_t *)(kind == 0 ? suffix : kind == 1 ? invalid : empty);
}
