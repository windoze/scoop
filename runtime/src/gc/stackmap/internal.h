#ifndef SCOOP_STACKMAP_INTERNAL_H
#define SCOOP_STACKMAP_INTERNAL_H
#include "../stackmap.h"
#include <string.h>

typedef struct ScoopStackMapCursor {
    const uint8_t *start;
    const uint8_t *current;
    const uint8_t *end;
    size_t image_index;
    ScoopStackMapError *error;
} ScoopStackMapCursor;

typedef struct ScoopStackMapFunction {
    uint64_t address;
    uint64_t stack_size;
    uint64_t record_count;
} ScoopStackMapFunction;

static inline bool fail(ScoopStackMapCursor *cursor, ScoopStackMapErrorCode code) {
    if (cursor->error->code == SCOOP_STACKMAP_OK) {
        cursor->error->code = code;
        cursor->error->image_index = cursor->image_index;
        cursor->error->section_offset = (size_t)(cursor->current - cursor->start);
    }
    return false;
}

static inline bool take(ScoopStackMapCursor *cursor, size_t count,
                        const uint8_t **bytes) {
    size_t remaining = (size_t)(cursor->end - cursor->current);
    if (count > remaining) {
        return fail(cursor, SCOOP_STACKMAP_TRUNCATED);
    }
    *bytes = cursor->current;
    cursor->current += count;
    return true;
}

static inline bool read_u8(ScoopStackMapCursor *cursor, uint8_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 1, &bytes)) {
        return false;
    }
    *value = bytes[0];
    return true;
}

static inline bool read_u16(ScoopStackMapCursor *cursor, uint16_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 2, &bytes)) {
        return false;
    }
    *value = (uint16_t)bytes[0] | (uint16_t)((uint16_t)bytes[1] << 8);
    return true;
}

static inline bool read_u32(ScoopStackMapCursor *cursor, uint32_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 4, &bytes)) {
        return false;
    }
    *value = (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8) |
             ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
    return true;
}

static inline bool read_u64(ScoopStackMapCursor *cursor, uint64_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 8, &bytes)) {
        return false;
    }
    *value = (uint64_t)bytes[0] | ((uint64_t)bytes[1] << 8) |
             ((uint64_t)bytes[2] << 16) | ((uint64_t)bytes[3] << 24) |
             ((uint64_t)bytes[4] << 32) | ((uint64_t)bytes[5] << 40) |
             ((uint64_t)bytes[6] << 48) | ((uint64_t)bytes[7] << 56);
    return true;
}

static inline bool read_i32(ScoopStackMapCursor *cursor, int32_t *value) {
    uint32_t bits;
    if (!read_u32(cursor, &bits)) {
        return false;
    }
    memcpy(value, &bits, sizeof bits);
    return true;
}

static inline bool read_zero_bytes(ScoopStackMapCursor *cursor, size_t count) {
    const uint8_t *bytes;
    if (!take(cursor, count, &bytes)) {
        return false;
    }
    for (size_t index = 0; index < count; index++) {
        if (bytes[index] != 0) {
            cursor->current = bytes + index;
            return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
        }
    }
    return true;
}

static inline bool align_cursor(ScoopStackMapCursor *cursor, size_t alignment) {
    size_t offset = (size_t)(cursor->current - cursor->start);
    size_t remainder = offset % alignment;
    return remainder == 0 || read_zero_bytes(cursor, alignment - remainder);
}

static inline bool checked_add_size(size_t left, size_t right, size_t *result) {
    if (right > SIZE_MAX - left) {
        return false;
    }
    *result = left + right;
    return true;
}

static inline bool checked_mul_size(size_t left, size_t right, size_t *result) {
    if (left != 0 && right > SIZE_MAX / left) {
        return false;
    }
    *result = left * right;
    return true;
}

bool scoop_stackmap_read_record(ScoopStackMapCursor *cursor,
                                const ScoopStackMapImage *image,
                                const ScoopStackMapFunction *function,
                                const uint64_t *constants, uint32_t constant_count,
                                ScoopStackMapRecord *record);
bool scoop_stackmap_read_image(const ScoopStackMapImage *image, size_t image_index,
                               ScoopStackMapIndex *index, ScoopStackMapError *error);

#endif
