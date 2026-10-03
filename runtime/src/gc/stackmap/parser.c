#include "internal.h"
#include <limits.h>
#include <stdlib.h>

#define SCOOP_STACKMAP_VERSION 3
#define SCOOP_STACKMAP_UNKNOWN_SIZE UINT64_MAX
#define SCOOP_STACK_ALIGNMENT UINT64_C(16)

static bool reserve_records(ScoopStackMapCursor *cursor, ScoopStackMapIndex *index,
                            size_t additional) {
    size_t total;
    size_t bytes;
    if (!checked_add_size(index->record_count, additional, &total) ||
        !checked_mul_size(total, sizeof *index->records, &bytes)) {
        return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
    }
    if (additional == 0) {
        return true;
    }
    ScoopStackMapRecord *records = realloc(index->records, bytes);
    if (records == NULL) {
        return fail(cursor, SCOOP_STACKMAP_OUT_OF_MEMORY);
    }
    index->records = records;
    memset(&index->records[index->record_count], 0, additional * sizeof *records);
    return true;
}

static bool read_contribution(const ScoopStackMapImage *image,
                              ScoopStackMapCursor *cursor, ScoopStackMapIndex *index) {
    uint8_t version;
    uint8_t reserved8;
    uint16_t reserved16;
    uint32_t function_count;
    uint32_t constant_count;
    uint32_t record_count;
    if (!read_u8(cursor, &version) || !read_u8(cursor, &reserved8) ||
        !read_u16(cursor, &reserved16) || !read_u32(cursor, &function_count) ||
        !read_u32(cursor, &constant_count) || !read_u32(cursor, &record_count)) {
        return false;
    }
    if (version != SCOOP_STACKMAP_VERSION) {
        return fail(cursor, SCOOP_STACKMAP_UNSUPPORTED_VERSION);
    }
    if (reserved8 != 0 || reserved16 != 0) {
        return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
    }
    if (function_count == 0 || record_count == 0) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_FUNCTION_COUNT);
    }

    size_t function_bytes;
    size_t constant_bytes;
    if (!checked_mul_size(function_count, sizeof(ScoopStackMapFunction),
                          &function_bytes) ||
        !checked_mul_size(constant_count, sizeof(uint64_t), &constant_bytes)) {
        return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
    }
    size_t prefix_bytes;
    if (!checked_add_size(function_bytes, constant_bytes, &prefix_bytes) ||
        prefix_bytes > (size_t)(cursor->end - cursor->current)) {
        return fail(cursor, SCOOP_STACKMAP_TRUNCATED);
    }
    ScoopStackMapFunction *functions = malloc(function_bytes);
    uint64_t *constants = constant_count == 0 ? NULL : malloc(constant_bytes);
    if (functions == NULL || (constant_count != 0 && constants == NULL)) {
        free(functions);
        free(constants);
        return fail(cursor, SCOOP_STACKMAP_OUT_OF_MEMORY);
    }

    uint64_t record_sum = 0;
    for (uint32_t index = 0; index < function_count; index++) {
        if (!read_u64(cursor, &functions[index].address) ||
            !read_u64(cursor, &functions[index].stack_size) ||
            !read_u64(cursor, &functions[index].record_count)) {
            free(functions);
            free(constants);
            return false;
        }
        if (functions[index].address < image->text_start ||
            functions[index].address >= image->text_end) {
            free(functions);
            free(constants);
            return fail(cursor, SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS);
        }
        if (functions[index].stack_size == SCOOP_STACKMAP_UNKNOWN_SIZE ||
            functions[index].stack_size == 0 ||
            functions[index].stack_size % SCOOP_STACK_ALIGNMENT != 0) {
            free(functions);
            free(constants);
            return fail(cursor, SCOOP_STACKMAP_INVALID_STACK_SIZE);
        }
        if (functions[index].record_count > UINT64_MAX - record_sum) {
            free(functions);
            free(constants);
            return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
        }
        record_sum += functions[index].record_count;
    }
    if (record_sum != record_count) {
        free(functions);
        free(constants);
        return fail(cursor, SCOOP_STACKMAP_INVALID_FUNCTION_COUNT);
    }
    for (uint32_t index = 0; index < constant_count; index++) {
        if (!read_u64(cursor, &constants[index])) {
            free(functions);
            free(constants);
            return false;
        }
    }
    if (record_count > (size_t)(cursor->end - cursor->current) / 64) {
        free(functions);
        free(constants);
        return fail(cursor, SCOOP_STACKMAP_TRUNCATED);
    }

    size_t old_count = index->record_count;
    if (!reserve_records(cursor, index, record_count)) {
        free(functions);
        free(constants);
        return false;
    }
    size_t record_index = old_count;
    for (uint32_t function_index = 0; function_index < function_count;
         function_index++) {
        for (uint64_t function_record = 0;
             function_record < functions[function_index].record_count;
             function_record++) {
            if (!scoop_stackmap_read_record(cursor, image, &functions[function_index],
                                            constants, constant_count,
                                            &index->records[record_index])) {
                index->record_count = record_index + 1;
                free(functions);
                free(constants);
                return false;
            }
            record_index++;
        }
    }
    index->record_count = record_index;
    free(functions);
    free(constants);

    return true;
}

bool scoop_stackmap_read_image(const ScoopStackMapImage *image, size_t image_index,
                               ScoopStackMapIndex *index, ScoopStackMapError *error) {
    if (image->section == NULL || image->section_size == 0 || image->text_start == 0 ||
        image->text_start >= image->text_end) {
        error->code = SCOOP_STACKMAP_INVALID_ARGUMENT;
        error->image_index = image_index;
        error->section_offset = 0;
        return false;
    }
    if (image->section_size > (size_t)PTRDIFF_MAX) {
        error->code = SCOOP_STACKMAP_INTEGER_OVERFLOW;
        error->image_index = image_index;
        error->section_offset = 0;
        return false;
    }
    ScoopStackMapCursor cursor = {
        .start = image->section,
        .current = image->section,
        .end = image->section + image->section_size,
        .image_index = image_index,
        .error = error,
    };

    do {
        if (!read_contribution(image, &cursor, index)) {
            return false;
        }
        if (cursor.current != cursor.end &&
            ((size_t)(cursor.end - cursor.current) < 16 ||
             cursor.current[0] != SCOOP_STACKMAP_VERSION)) {
            return fail(&cursor, SCOOP_STACKMAP_TRAILING_DATA);
        }
    } while (cursor.current != cursor.end);
    return true;
}
