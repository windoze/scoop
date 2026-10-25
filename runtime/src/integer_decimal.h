#ifndef SCOOP_INTEGER_DECIMAL_H
#define SCOOP_INTEGER_DECIMAL_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "ryu/digit_table.h"

/* Write backwards into twenty bytes, without a NUL. The returned length
 * selects buffer[20 - length .. 20]. Digit pairs reuse the vendored Ryu table;
 * eight-digit chunks keep most arithmetic in 32 bits, as in Ryu's d2s writer. */
static inline size_t scoop_format_unsigned_decimal(uint64_t value, char buffer[20]) {
    char *cursor = buffer + 20;
    while (value >= UINT64_C(100000000)) {
        uint64_t quotient = value / UINT64_C(100000000);
        uint32_t remainder = (uint32_t)(value - quotient * UINT64_C(100000000));
        uint32_t low = remainder % 10000;
        uint32_t high = remainder / 10000;
        cursor -= 8;
        memcpy(cursor + 6, DIGIT_TABLE + 2 * (low % 100), 2);
        memcpy(cursor + 4, DIGIT_TABLE + 2 * (low / 100), 2);
        memcpy(cursor + 2, DIGIT_TABLE + 2 * (high % 100), 2);
        memcpy(cursor, DIGIT_TABLE + 2 * (high / 100), 2);
        value = quotient;
    }
    uint32_t tail = (uint32_t)value;
    while (tail >= 100) {
        uint32_t quotient = tail / 100;
        cursor -= 2;
        memcpy(cursor, DIGIT_TABLE + 2 * (tail - quotient * 100), 2);
        tail = quotient;
    }
    if (tail < 10) {
        *--cursor = (char)('0' + tail);
    } else {
        cursor -= 2;
        memcpy(cursor, DIGIT_TABLE + 2 * tail, 2);
    }
    return (size_t)(buffer + 20 - cursor);
}

static inline size_t scoop_format_signed_decimal(int64_t value, char buffer[20]) {
    uint64_t magnitude = (uint64_t)value;
    if (value < 0) {
        magnitude = UINT64_C(0) - magnitude;
    }
    size_t length = scoop_format_unsigned_decimal(magnitude, buffer);
    if (value < 0) {
        buffer[20 - ++length] = '-';
    }
    return length;
}

#endif
