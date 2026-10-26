#include <string.h>

#include "scoop_rt.h"
#include "value_shape.h"

static uint32_t decode_scalar(const unsigned char *bytes) {
    uint32_t first = bytes[0];
    if (first <= 0x7f) {
        return first;
    }
    if (first <= 0xdf) {
        return ((first & 0x1f) << 6) | (bytes[1] & 0x3f);
    }
    if (first <= 0xef) {
        return ((first & 0x0f) << 12) | ((bytes[1] & 0x3f) << 6) | (bytes[2] & 0x3f);
    }
    return ((first & 0x07) << 18) | ((bytes[1] & 0x3f) << 12) | ((bytes[2] & 0x3f) << 6) |
           (bytes[3] & 0x3f);
}

int64_t scoop_rt_string_byte_length_leaf(const ScoopString *value) { return (int64_t)value->len; }

uint32_t scoop_rt_string_character_at_byte_leaf(const ScoopString *value, int64_t index) {
    if (index < 0 || (uint64_t)index >= value->len) {
        scoop_shape_fatal("String byte cursor out of bounds");
    }
    return decode_scalar((const unsigned char *)value->data + index);
}

int8_t scoop_rt_string_byte_at_leaf(const ScoopString *value, int64_t index) {
    if (index < 0 || (uint64_t)index >= value->len) {
        scoop_shape_fatal("String byte index out of bounds");
    }
    int8_t result;
    memcpy(&result, value->data + index, sizeof(result));
    return result;
}
