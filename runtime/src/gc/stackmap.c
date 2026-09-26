#include "stackmap.h"

#include <limits.h>
#include <stdlib.h>
#include <string.h>

#define SCOOP_STACKMAP_VERSION 3
#define SCOOP_STACKMAP_UNKNOWN_SIZE UINT64_MAX
#define SCOOP_STACK_ALIGNMENT UINT64_C(16)

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

static bool fail(ScoopStackMapCursor *cursor, ScoopStackMapErrorCode code) {
    if (cursor->error->code == SCOOP_STACKMAP_OK) {
        cursor->error->code = code;
        cursor->error->image_index = cursor->image_index;
        cursor->error->section_offset =
            (size_t)(cursor->current - cursor->start);
    }
    return false;
}

static bool take(ScoopStackMapCursor *cursor, size_t count,
                 const uint8_t **bytes) {
    size_t remaining = (size_t)(cursor->end - cursor->current);
    if (count > remaining) {
        return fail(cursor, SCOOP_STACKMAP_TRUNCATED);
    }
    *bytes = cursor->current;
    cursor->current += count;
    return true;
}

static bool read_u8(ScoopStackMapCursor *cursor, uint8_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 1, &bytes)) {
        return false;
    }
    *value = bytes[0];
    return true;
}

static bool read_u16(ScoopStackMapCursor *cursor, uint16_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 2, &bytes)) {
        return false;
    }
    *value = (uint16_t)bytes[0] | (uint16_t)((uint16_t)bytes[1] << 8);
    return true;
}

static bool read_u32(ScoopStackMapCursor *cursor, uint32_t *value) {
    const uint8_t *bytes;
    if (!take(cursor, 4, &bytes)) {
        return false;
    }
    *value = (uint32_t)bytes[0] | ((uint32_t)bytes[1] << 8) |
             ((uint32_t)bytes[2] << 16) | ((uint32_t)bytes[3] << 24);
    return true;
}

static bool read_u64(ScoopStackMapCursor *cursor, uint64_t *value) {
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

static bool read_i32(ScoopStackMapCursor *cursor, int32_t *value) {
    uint32_t bits;
    if (!read_u32(cursor, &bits)) {
        return false;
    }
    memcpy(value, &bits, sizeof bits);
    return true;
}

static bool read_zero_bytes(ScoopStackMapCursor *cursor, size_t count) {
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

static bool align_cursor(ScoopStackMapCursor *cursor, size_t alignment) {
    size_t offset = (size_t)(cursor->current - cursor->start);
    size_t remainder = offset % alignment;
    return remainder == 0 || read_zero_bytes(cursor, alignment - remainder);
}

static bool checked_add_size(size_t left, size_t right, size_t *result) {
    if (right > SIZE_MAX - left) {
        return false;
    }
    *result = left + right;
    return true;
}

static bool checked_mul_size(size_t left, size_t right, size_t *result) {
    if (left != 0 && right > SIZE_MAX / left) {
        return false;
    }
    *result = left * right;
    return true;
}

static bool reserve_records(ScoopStackMapCursor *cursor,
                            ScoopStackMapIndex *index, size_t additional) {
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
    memset(&index->records[index->record_count], 0,
           additional * sizeof *records);
    return true;
}

static bool read_location(ScoopStackMapCursor *cursor,
                          const uint64_t *constants, uint32_t constant_count,
                          ScoopStackMapLocation *location) {
    uint8_t kind;
    uint8_t reserved8;
    uint16_t reserved16;
    if (!read_u8(cursor, &kind) || !read_u8(cursor, &reserved8) ||
        !read_u16(cursor, &location->size) ||
        !read_u16(cursor, &location->dwarf_register) ||
        !read_u16(cursor, &reserved16) ||
        !read_i32(cursor, &location->offset)) {
        return false;
    }
    if (reserved8 != 0 || reserved16 != 0) {
        return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
    }
    if (kind < SCOOP_STACKMAP_REGISTER ||
        kind > SCOOP_STACKMAP_CONSTANT_INDEX) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_LOCATION);
    }
    location->kind = (ScoopStackMapLocationKind)kind;
    location->constant = 0;
    if (location->kind == SCOOP_STACKMAP_CONSTANT) {
        location->constant = (uint64_t)(int64_t)location->offset;
    } else if (location->kind == SCOOP_STACKMAP_CONSTANT_INDEX) {
        if (location->offset < 0 ||
            (uint32_t)location->offset >= constant_count) {
            return fail(cursor, SCOOP_STACKMAP_INVALID_CONSTANT_INDEX);
        }
        location->constant = constants[(uint32_t)location->offset];
    }
    return true;
}

static bool read_statepoint_header(ScoopStackMapCursor *cursor,
                                   const ScoopStackMapLocation header[3]) {
    for (size_t index = 0; index < 3; index++) {
        if ((header[index].kind != SCOOP_STACKMAP_CONSTANT &&
             header[index].kind != SCOOP_STACKMAP_CONSTANT_INDEX) ||
            header[index].size != sizeof(uint64_t)) {
            return fail(cursor, SCOOP_STACKMAP_INVALID_RECORD);
        }
    }
    if (header[1].constant != 0) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_RECORD);
    }
    if (header[2].constant != 0) {
        return fail(cursor, SCOOP_STACKMAP_DEOPT_UNSUPPORTED);
    }
    return true;
}

static bool read_record(ScoopStackMapCursor *cursor,
                        const ScoopStackMapImage *image,
                        const ScoopStackMapFunction *function,
                        const uint64_t *constants, uint32_t constant_count,
                        ScoopStackMapRecord *record) {
    uint32_t instruction_offset;
    uint16_t flags;
    uint16_t location_count;
    if (!read_u64(cursor, &record->safepoint_id) ||
        !read_u32(cursor, &instruction_offset) || !read_u16(cursor, &flags) ||
        !read_u16(cursor, &location_count)) {
        return false;
    }
    if (record->safepoint_id == 0 || flags != 0 || location_count < 3 ||
        ((uint32_t)location_count - 3) % 2 != 0) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_RECORD);
    }
    if (instruction_offset > UINTPTR_MAX - function->address) {
        return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
    }
    record->function_address = (uintptr_t)function->address;
    record->return_pc =
        (uintptr_t)function->address + (uintptr_t)instruction_offset;
    record->stack_size = function->stack_size;
    if (record->function_address < image->text_start ||
        record->function_address >= image->text_end ||
        record->return_pc < image->text_start ||
        record->return_pc >= image->text_end) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS);
    }

    ScoopStackMapLocation header[3];
    for (size_t index = 0; index < 3; index++) {
        if (!read_location(cursor, constants, constant_count, &header[index])) {
            return false;
        }
    }
    if (!read_statepoint_header(cursor, header)) {
        return false;
    }

    record->root_count = (uint16_t)((location_count - 3) / 2);
    if (record->root_count != 0) {
        size_t bytes;
        if (!checked_mul_size(record->root_count, sizeof *record->roots,
                              &bytes)) {
            return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
        }
        record->roots = calloc(1, bytes);
        if (record->roots == NULL) {
            return fail(cursor, SCOOP_STACKMAP_OUT_OF_MEMORY);
        }
    }
    for (uint16_t index = 0; index < record->root_count; index++) {
        if (!read_location(cursor, constants, constant_count,
                           &record->roots[index].base) ||
            !read_location(cursor, constants, constant_count,
                           &record->roots[index].derived)) {
            return false;
        }
    }
    if (!align_cursor(cursor, 8)) {
        return false;
    }

    uint16_t padding;
    uint16_t live_out_count;
    if (!read_u16(cursor, &padding) || !read_u16(cursor, &live_out_count)) {
        return false;
    }
    if (padding != 0) {
        return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
    }
    for (uint16_t index = 0; index < live_out_count; index++) {
        uint16_t dwarf_register;
        uint8_t reserved;
        uint8_t size;
        if (!read_u16(cursor, &dwarf_register) ||
            !read_u8(cursor, &reserved) || !read_u8(cursor, &size)) {
            return false;
        }
        (void)dwarf_register;
        (void)size;
        if (reserved != 0) {
            return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
        }
    }
    return align_cursor(cursor, 8);
}

static bool read_contribution(const ScoopStackMapImage *image,
                               ScoopStackMapCursor *cursor,
                               ScoopStackMapIndex *index) {
    uint8_t version;
    uint8_t reserved8;
    uint16_t reserved16;
    uint32_t function_count;
    uint32_t constant_count;
    uint32_t record_count;
    if (!read_u8(cursor, &version) || !read_u8(cursor, &reserved8) ||
        !read_u16(cursor, &reserved16) ||
        !read_u32(cursor, &function_count) ||
        !read_u32(cursor, &constant_count) ||
        !read_u32(cursor, &record_count)) {
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
            if (!read_record(cursor, image, &functions[function_index],
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

static bool read_image(const ScoopStackMapImage *image, size_t image_index,
                       ScoopStackMapIndex *index, ScoopStackMapError *error) {
    if (image->section == NULL || image->section_size == 0 ||
        image->text_start == 0 || image->text_start >= image->text_end) {
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

static int compare_record_pc(const void *left, const void *right) {
    const ScoopStackMapRecord *left_record = left;
    const ScoopStackMapRecord *right_record = right;
    if (left_record->return_pc < right_record->return_pc) {
        return -1;
    }
    if (left_record->return_pc > right_record->return_pc) {
        return 1;
    }
    return 0;
}

static int compare_record_id(const void *left, const void *right) {
    const ScoopStackMapRecord *left_record = left;
    const ScoopStackMapRecord *right_record = right;
    if (left_record->safepoint_id < right_record->safepoint_id) {
        return -1;
    }
    if (left_record->safepoint_id > right_record->safepoint_id) {
        return 1;
    }
    return 0;
}

bool scoop_stackmap_build_index(const ScoopStackMapImage *images,
                                size_t image_count,
                                ScoopStackMapIndex *index,
                                ScoopStackMapError *error) {
    if (index == NULL || error == NULL) {
        return false;
    }
    *index = (ScoopStackMapIndex){0};
    *error = (ScoopStackMapError){.code = SCOOP_STACKMAP_OK};
    if (images == NULL || image_count == 0) {
        error->code = SCOOP_STACKMAP_INVALID_ARGUMENT;
        return false;
    }
    for (size_t image = 0; image < image_count; image++) {
        if (!read_image(&images[image], image, index, error)) {
            scoop_stackmap_dispose_index(index);
            return false;
        }
    }
    qsort(index->records, index->record_count, sizeof *index->records,
          compare_record_id);
    for (size_t record = 1; record < index->record_count; record++) {
        if (index->records[record - 1].safepoint_id ==
            index->records[record].safepoint_id) {
            error->code = SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID;
            scoop_stackmap_dispose_index(index);
            return false;
        }
    }
    qsort(index->records, index->record_count, sizeof *index->records,
          compare_record_pc);
    for (size_t record = 1; record < index->record_count; record++) {
        if (index->records[record - 1].return_pc ==
            index->records[record].return_pc) {
            error->code = SCOOP_STACKMAP_DUPLICATE_RETURN_PC;
            scoop_stackmap_dispose_index(index);
            return false;
        }
    }
    return true;
}

void scoop_stackmap_dispose_index(ScoopStackMapIndex *index) {
    if (index == NULL) {
        return;
    }
    for (size_t record = 0; record < index->record_count; record++) {
        free(index->records[record].roots);
    }
    free(index->records);
    *index = (ScoopStackMapIndex){0};
}

const ScoopStackMapRecord *
scoop_stackmap_lookup(const ScoopStackMapIndex *index, uintptr_t return_pc) {
    if (index == NULL) {
        return NULL;
    }
    size_t low = 0;
    size_t high = index->record_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2;
        uintptr_t candidate = index->records[middle].return_pc;
        if (candidate < return_pc) {
            low = middle + 1;
        } else if (candidate > return_pc) {
            high = middle;
        } else {
            return &index->records[middle];
        }
    }
    return NULL;
}

const char *scoop_stackmap_error_message(ScoopStackMapErrorCode code) {
    switch (code) {
    case SCOOP_STACKMAP_OK:
        return "no error";
    case SCOOP_STACKMAP_INVALID_ARGUMENT:
        return "invalid parser argument or image span";
    case SCOOP_STACKMAP_TRUNCATED:
        return "truncated stack map section";
    case SCOOP_STACKMAP_UNSUPPORTED_VERSION:
        return "unsupported stack map version";
    case SCOOP_STACKMAP_NONZERO_RESERVED:
        return "non-zero reserved or padding field";
    case SCOOP_STACKMAP_INTEGER_OVERFLOW:
        return "stack map size or address overflow";
    case SCOOP_STACKMAP_OUT_OF_MEMORY:
        return "out of memory building stack map index";
    case SCOOP_STACKMAP_INVALID_FUNCTION_COUNT:
        return "function and record counts disagree";
    case SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS:
        return "function or return PC is outside executable text";
    case SCOOP_STACKMAP_INVALID_STACK_SIZE:
        return "dynamic, zero, or unaligned stack size";
    case SCOOP_STACKMAP_INVALID_RECORD:
        return "invalid statepoint stack map record";
    case SCOOP_STACKMAP_INVALID_LOCATION:
        return "unknown stack map location kind";
    case SCOOP_STACKMAP_INVALID_CONSTANT_INDEX:
        return "constant-pool location index is invalid";
    case SCOOP_STACKMAP_DEOPT_UNSUPPORTED:
        return "statepoint deopt locations are unsupported";
    case SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID:
        return "duplicate SafepointId";
    case SCOOP_STACKMAP_DUPLICATE_RETURN_PC:
        return "duplicate statepoint return PC";
    case SCOOP_STACKMAP_TRAILING_DATA:
        return "unexpected trailing stack map data";
    }
    return "unknown stack map parser error";
}
