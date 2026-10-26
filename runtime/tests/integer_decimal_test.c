#include <assert.h>
#include <limits.h>
#include <stdio.h>

#include "integer_decimal.h"

static void check(uint64_t value) {
    char expected[24];
    unsigned char storage[22];
    char *buffer = (char *)storage + 1;
    memset(storage, 0x55, sizeof storage);
    size_t length = scoop_format_unsigned_decimal(value, buffer);
    int count = snprintf(expected, sizeof expected, "%llu", (unsigned long long)value);
    assert(length >= 1 && length <= 20 && length == (size_t)count);
    assert(memcmp(buffer + 20 - length, expected, length) == 0);
    for (size_t index = 0; index < 21 - length; ++index) {
        assert(storage[index] == 0x55);
    }
    assert(storage[21] == 0x55);

    int64_t signed_value;
    memcpy(&signed_value, &value, sizeof value);
    memset(storage, 0x55, sizeof storage);
    length = scoop_format_signed_decimal(signed_value, buffer);
    count = snprintf(expected, sizeof expected, "%lld", (long long)signed_value);
    assert(length >= 1 && length <= 20 && length == (size_t)count);
    assert(memcmp(buffer + 20 - length, expected, length) == 0);
    for (size_t index = 0; index < 21 - length; ++index) {
        assert(storage[index] == 0x55);
    }
    assert(storage[21] == 0x55);
}

int main(void) {
    check(0);
    check(UINT64_MAX);
    check((uint64_t)INT64_MIN);
    check((uint64_t)INT64_MAX);
    for (uint64_t power = 1;; power *= 10) {
        for (uint64_t value = power - 1; value <= power + 1; ++value) {
            check(value);
            check(UINT64_C(0) - value);
        }
        if (power > UINT64_MAX / 10) {
            break;
        }
    }
    uint64_t random = 42;
    for (uint64_t value = 0; value < 100000; ++value) {
        check(value);
        random = random * UINT64_C(6364136223846793005) + 1;
        check(random);
    }
    puts("integer decimal tests passed");
}
