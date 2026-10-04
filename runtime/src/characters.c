/* Encoding a checked Unicode scalar never requires managed input roots. */
#include <string.h>

#include "scoop_rt.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

const ScoopString *scoop_rt_char_to_string(uint32_t value) {
    unsigned char bytes[4];
    uint64_t length;
    if (value <= 0x7f) {
        bytes[0] = (unsigned char)value;
        length = 1;
    } else if (value <= 0x7ff) {
        bytes[0] = (unsigned char)(0xc0 | (value >> 6));
        bytes[1] = (unsigned char)(0x80 | (value & 0x3f));
        length = 2;
    } else if (value <= 0xffff) {
        bytes[0] = (unsigned char)(0xe0 | (value >> 12));
        bytes[1] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | (value & 0x3f));
        length = 3;
    } else {
        bytes[0] = (unsigned char)(0xf0 | (value >> 18));
        bytes[1] = (unsigned char)(0x80 | ((value >> 12) & 0x3f));
        bytes[2] = (unsigned char)(0x80 | ((value >> 6) & 0x3f));
        bytes[3] = (unsigned char)(0x80 | (value & 0x3f));
        length = 4;
    }
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    memcpy(result->data, bytes, (size_t)length);
    return result;
}
