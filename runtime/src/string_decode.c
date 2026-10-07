#include <string.h>

#include "string_abi.h"
#include "utf8.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

static uint64_t checked_length(const unsigned char *bytes, int64_t length) {
    if (bytes == NULL || length < 0) {
        scoop_shape_fatal("invalid UTF-8 decode range");
    }
    return (uint64_t)length;
}

/* Byte storage is fixed by the caller's scoped borrow or native contract.
 * Allocation may collect; no interior managed pointer is retained here. */
void scoop_rt_string_decode_utf8_storage(ScoopStringDecodeResult *result,
                                         const unsigned char *bytes, int64_t length) {
    const uint64_t count = checked_length(bytes, length);
    for (uint64_t cursor = 0; cursor < count;) {
        const ScoopUtf8Step step = scoop_utf8_next(bytes + cursor, count - cursor);
        if (!step.valid) {
            *result = (ScoopStringDecodeResult){.value = NULL, .invalid_offset = (int64_t)cursor};
            return;
        }
        cursor += step.consumed;
    }
    ScoopString *value =
        scoop_rt_alloc(&scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, count));
    value->len = count;
    if (count != 0) {
        memcpy(value->data, bytes, (size_t)count);
    }
    *result = (ScoopStringDecodeResult){.value = value, .invalid_offset = -1};
}

const ScoopString *scoop_rt_string_decode_utf8_lossy(const unsigned char *bytes, int64_t length) {
    const uint64_t count = checked_length(bytes, length);
    uint64_t output_length = 0;
    for (uint64_t cursor = 0; cursor < count;) {
        const ScoopUtf8Step step = scoop_utf8_next(bytes + cursor, count - cursor);
        output_length = scoop_shape_add(output_length, step.valid ? step.consumed : 3);
        cursor += step.consumed;
    }
    ScoopString *value = scoop_rt_alloc(
        &scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, output_length));
    value->len = output_length;
    uint64_t output = 0;
    for (uint64_t cursor = 0; cursor < count;) {
        const ScoopUtf8Step step = scoop_utf8_next(bytes + cursor, count - cursor);
        if (step.valid) {
            memcpy(value->data + output, bytes + cursor, step.consumed);
            output += step.consumed;
        } else {
            output += scoop_utf8_encode((unsigned char *)value->data + output, 0xfffd);
        }
        cursor += step.consumed;
    }
    return value;
}
