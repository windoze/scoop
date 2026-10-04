#include "utf8.h"

uint64_t scoop_utf8_width(uint32_t value) {
    return value <= 0x7f ? 1 : value <= 0x7ff ? 2 : value <= 0xffff ? 3 : 4;
}

uint64_t scoop_utf8_encode(unsigned char *bytes, uint32_t value) {
    uint64_t width = scoop_utf8_width(value);
    if (width == 1) {
        bytes[0] = (unsigned char)value;
    } else if (width == 2) {
        bytes[0] = (unsigned char)(0xc0 | (value >> 6));
        bytes[1] = (unsigned char)(0x80 | (value & 0x3f));
    } else if (width == 3) {
        bytes[0] = (unsigned char)(0xe0 | (value >> 12));
        bytes[1] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | (value & 0x3f));
    } else {
        bytes[0] = (unsigned char)(0xf0 | (value >> 18));
        bytes[1] = (unsigned char)(0x80 | ((value >> 12) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[3] = (unsigned char)(0x80 | (value & 0x3f));
    }
    return width;
}

uint32_t scoop_utf8_decode(const unsigned char *bytes) {
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
