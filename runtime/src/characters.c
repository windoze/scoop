/* Encoding a checked Unicode scalar never requires managed input roots. */
#include <string.h>

#include "scoop_rt.h"
#include "utf8.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

const ScoopString *scoop_rt_char_to_string(uint32_t value) {
    unsigned char bytes[4];
    uint64_t length = scoop_utf8_encode(bytes, value);
    ScoopString *result = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    memcpy(result->data, bytes, (size_t)length);
    return result;
}
