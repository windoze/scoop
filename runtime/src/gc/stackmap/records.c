#include "internal.h"
#include <stdlib.h>

static bool read_location(ScoopStackMapCursor *cursor, const uint64_t *constants,
                          uint32_t constant_count, ScoopStackMapLocation *location) {
    uint8_t kind;
    uint8_t reserved8;
    uint16_t reserved16;
    if (!read_u8(cursor, &kind) || !read_u8(cursor, &reserved8) ||
        !read_u16(cursor, &location->size) ||
        !read_u16(cursor, &location->dwarf_register) ||
        !read_u16(cursor, &reserved16) || !read_i32(cursor, &location->offset)) {
        return false;
    }
    if (reserved8 != 0 || reserved16 != 0) {
        return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
    }
    if (kind < SCOOP_STACKMAP_REGISTER || kind > SCOOP_STACKMAP_CONSTANT_INDEX) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_LOCATION);
    }
    location->kind = (ScoopStackMapLocationKind)kind;
    location->constant = 0;
    if ((location->kind == SCOOP_STACKMAP_REGISTER && location->offset != 0) ||
        ((location->kind == SCOOP_STACKMAP_CONSTANT ||
          location->kind == SCOOP_STACKMAP_CONSTANT_INDEX) &&
         location->dwarf_register != 0)) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_LOCATION);
    }
    if (location->kind == SCOOP_STACKMAP_CONSTANT) {
        location->constant = (uint64_t)(int64_t)location->offset;
    } else if (location->kind == SCOOP_STACKMAP_CONSTANT_INDEX) {
        if (location->offset < 0 || (uint32_t)location->offset >= constant_count) {
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

bool scoop_stackmap_read_record(ScoopStackMapCursor *cursor,
                                const ScoopStackMapImage *image,
                                const ScoopStackMapFunction *function,
                                const uint64_t *constants, uint32_t constant_count,
                                ScoopStackMapRecord *record) {
    record->image_index = cursor->image_index;
    record->section_offset = (size_t)(cursor->current - cursor->start);
    uint16_t flags;
    uint16_t location_count;
    if (!read_u64(cursor, &record->safepoint_id) ||
        !read_u32(cursor, &record->instruction_offset) || !read_u16(cursor, &flags) ||
        !read_u16(cursor, &location_count)) {
        return false;
    }
    if (record->safepoint_id == 0 || flags != 0 || location_count < 3 ||
        ((uint32_t)location_count - 3) % 2 != 0) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_RECORD);
    }
    if (record->instruction_offset > UINTPTR_MAX - function->address) {
        return fail(cursor, SCOOP_STACKMAP_INTEGER_OVERFLOW);
    }
    record->function_address = (uintptr_t)function->address;
    record->return_pc =
        (uintptr_t)function->address + (uintptr_t)record->instruction_offset;
    record->stack_size = function->stack_size;
    if (record->function_address < image->text_start ||
        record->function_address >= image->text_end ||
        record->return_pc < image->text_start || record->return_pc >= image->text_end) {
        return fail(cursor, SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS);
    }

    for (size_t index = 0; index < 3; index++) {
        if (!read_location(cursor, constants, constant_count, &record->header[index])) {
            return false;
        }
    }
    if (!read_statepoint_header(cursor, record->header)) {
        return false;
    }

    record->root_count = (uint16_t)((location_count - 3) / 2);
    if (record->root_count != 0) {
        size_t bytes;
        if (!checked_mul_size(record->root_count, sizeof *record->roots, &bytes)) {
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
    record->live_out_count = live_out_count;
    if (live_out_count != 0) {
        record->live_outs = calloc(live_out_count, sizeof *record->live_outs);
        if (record->live_outs == NULL) {
            return fail(cursor, SCOOP_STACKMAP_OUT_OF_MEMORY);
        }
    }
    for (uint16_t index = 0; index < live_out_count; index++) {
        uint8_t reserved;
        if (!read_u16(cursor, &record->live_outs[index].dwarf_register) ||
            !read_u8(cursor, &reserved) ||
            !read_u8(cursor, &record->live_outs[index].size)) {
            return false;
        }
        if (reserved != 0) {
            return fail(cursor, SCOOP_STACKMAP_NONZERO_RESERVED);
        }
    }
    return align_cursor(cursor, 8);
}
