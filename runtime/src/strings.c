#include <string.h>

#include "string_abi.h"
#include "utf8.h"
#include "value_shape.h"

extern const ScoopTypeDescriptor scoop_td_String;

int64_t scoop_rt_string_byte_length(const ScoopString *value) { return (int64_t)value->len; }

int64_t scoop_rt_string_length(const ScoopString *value) {
    int64_t length = 0;
    for (uint64_t cursor = 0; cursor < value->len; ++cursor) {
        if (((unsigned char)value->data[cursor] & 0xc0) != 0x80) {
            ++length;
        }
    }
    return length;
}

uint32_t scoop_rt_string_character_at_byte(const ScoopString *value, int64_t index) {
    if (index < 0 || (uint64_t)index >= value->len) {
        scoop_shape_fatal("String byte cursor out of bounds");
    }
    return scoop_utf8_decode((const unsigned char *)value->data + index);
}

int8_t scoop_rt_string_byte_at(const ScoopString *value, int64_t index) {
    if (index < 0 || (uint64_t)index >= value->len) {
        scoop_shape_fatal("String byte index out of bounds");
    }
    int8_t result;
    memcpy(&result, value->data + index, sizeof(result));
    return result;
}

ScoopStringCharResult scoop_rt_string_get(const ScoopString *value, int64_t index) {
    ScoopStringCharResult result = {.tag = 1};
    if (index < 0) {
        return result;
    }
    uint64_t cursor = 0;
    for (int64_t scalar = 0; cursor < value->len; ++scalar) {
        uint32_t character = scoop_utf8_decode((const unsigned char *)value->data + cursor);
        if (scalar == index) {
            result.tag = 0;
            result.value = character;
            return result;
        }
        cursor += scoop_utf8_width(character);
    }
    return result;
}

void scoop_rt_string_slice_bounds_storage(ScoopStringBoundsResult *result, const ScoopString *value,
                                          int64_t start, int64_t end) {
    *result = (ScoopStringBoundsResult){.tag = 1};
    if (start < 0 || end < start) {
        return;
    }
    uint64_t cursor = 0;
    uint64_t start_byte = 0;
    int64_t scalar = 0;
    for (;;) {
        if (scalar == start) {
            start_byte = cursor;
        }
        if (scalar == end) {
            result->tag = 0;
            result->start = (int64_t)start_byte;
            result->end = (int64_t)cursor;
            return;
        }
        if (cursor == value->len) {
            return;
        }
        cursor += scoop_utf8_width(scoop_utf8_decode((const unsigned char *)value->data + cursor));
        ++scalar;
    }
}

const ScoopString *scoop_rt_string_slice_bytes(const ScoopString *value, int64_t start,
                                               int64_t end) {
    if (start < 0 || end < start || (uint64_t)end > value->len) {
        scoop_shape_fatal("String slice byte range out of bounds");
    }
    const ScoopString *source = value;
    void **slots[] = {(void **)&source};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    uint64_t length = (uint64_t)(end - start);
    ScoopString *result =
        scoop_rt_alloc(&scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    memcpy(result->data, source->data + start, (size_t)length);
    scoop_rt_pop_native_roots(&roots);
    return result;
}

const ScoopString *scoop_rt_string_from_bytes(const ScoopArray *value) {
    const ScoopTypeInstanceShapeV1 *shape =
        scoop_shape_require(value->header.td, SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    const ScoopArray *source = value;
    uint64_t length = source->size;
    void **slots[] = {(void **)&source};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    ScoopString *result =
        scoop_rt_alloc(&scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    memcpy(result->data, (const char *)source + shape->inline_offset, (size_t)length);
    scoop_rt_pop_native_roots(&roots);
    return result;
}

const ScoopString *scoop_rt_string_from_chars(const ScoopArray *value) {
    const ScoopTypeInstanceShapeV1 *shape =
        scoop_shape_require(value->header.td, SCOOP_TYPE_INSTANCE_INLINE_ARRAY_V1);
    const ScoopArray *source = value;
    uint64_t count = source->size;
    const uint32_t *characters = (const uint32_t *)((const char *)source + shape->inline_offset);
    uint64_t length = 0;
    for (uint64_t index = 0; index < count; ++index) {
        length = scoop_shape_add(length, scoop_utf8_width(characters[index]));
    }
    void **slots[] = {(void **)&source};
    ScoopNativeRootFrame roots;
    scoop_rt_push_native_roots(&roots, slots, 1);
    ScoopString *result =
        scoop_rt_alloc(&scoop_td_String, scoop_shape_allocation_size(&scoop_td_String, length));
    result->len = length;
    characters = (const uint32_t *)((const char *)source + shape->inline_offset);
    uint64_t cursor = 0;
    for (uint64_t index = 0; index < count; ++index) {
        cursor += scoop_utf8_encode((unsigned char *)result->data + cursor, characters[index]);
    }
    scoop_rt_pop_native_roots(&roots);
    return result;
}
