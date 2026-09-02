#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../src/gc/stackmap.h"
#include "../src/platform/platform.h"

extern const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops;

typedef struct TestBuffer {
    uint8_t bytes[512];
    size_t length;
    size_t first_record;
    size_t first_padding;
    size_t second_record;
    size_t constant_index_location;
} TestBuffer;

static void write_u8(TestBuffer *buffer, uint8_t value) {
    assert(buffer->length < sizeof buffer->bytes);
    buffer->bytes[buffer->length++] = value;
}

static void write_u16(TestBuffer *buffer, uint16_t value) {
    write_u8(buffer, (uint8_t)value);
    write_u8(buffer, (uint8_t)(value >> 8));
}

static void write_u32(TestBuffer *buffer, uint32_t value) {
    write_u16(buffer, (uint16_t)value);
    write_u16(buffer, (uint16_t)(value >> 16));
}

static void write_u64(TestBuffer *buffer, uint64_t value) {
    write_u32(buffer, (uint32_t)value);
    write_u32(buffer, (uint32_t)(value >> 32));
}

static void write_i32(TestBuffer *buffer, int32_t value) {
    uint32_t bits;
    memcpy(&bits, &value, sizeof bits);
    write_u32(buffer, bits);
}

static void write_location(TestBuffer *buffer, uint8_t kind, uint16_t size,
                           uint16_t dwarf_register, int32_t offset) {
    write_u8(buffer, kind);
    write_u8(buffer, 0);
    write_u16(buffer, size);
    write_u16(buffer, dwarf_register);
    write_u16(buffer, 0);
    write_i32(buffer, offset);
}

static void align_buffer(TestBuffer *buffer) {
    while (buffer->length % 8 != 0) {
        write_u8(buffer, 0);
    }
}

static void write_record_end(TestBuffer *buffer, uint16_t live_out_count) {
    align_buffer(buffer);
    write_u16(buffer, 0);
    write_u16(buffer, live_out_count);
    for (uint16_t index = 0; index < live_out_count; index++) {
        write_u16(buffer, (uint16_t)(19 + index));
        write_u8(buffer, 0);
        write_u8(buffer, 8);
    }
    align_buffer(buffer);
}

static void write_statepoint_header(TestBuffer *buffer) {
    write_location(buffer, SCOOP_STACKMAP_CONSTANT, 8, 0, 0);
    write_location(buffer, SCOOP_STACKMAP_CONSTANT, 8, 0, 0);
    write_location(buffer, SCOOP_STACKMAP_CONSTANT, 8, 0, 0);
}

static TestBuffer valid_section(void) {
    TestBuffer buffer = {0};
    write_u8(&buffer, 3);
    write_u8(&buffer, 0);
    write_u16(&buffer, 0);
    write_u32(&buffer, 1); /* functions */
    write_u32(&buffer, 1); /* constants */
    write_u32(&buffer, 2); /* records */

    write_u64(&buffer, 0x1000); /* function address */
    write_u64(&buffer, 64); /* stack size */
    write_u64(&buffer, 2); /* records in function */
    write_u64(&buffer, UINT64_C(0x1122334455667788));

    buffer.first_record = buffer.length;
    write_u64(&buffer, 9);
    write_u32(&buffer, 0x20);
    write_u16(&buffer, 0);
    write_u16(&buffer, 3);
    write_statepoint_header(&buffer);
    buffer.first_padding = buffer.length;
    write_record_end(&buffer, 1);

    buffer.second_record = buffer.length;
    write_u64(&buffer, 7);
    write_u32(&buffer, 0x10);
    write_u16(&buffer, 0);
    write_u16(&buffer, 13);
    write_statepoint_header(&buffer);
    write_location(&buffer, SCOOP_STACKMAP_REGISTER, 8, 0, 0);
    write_location(&buffer, SCOOP_STACKMAP_REGISTER, 8, 0, 0);
    write_location(&buffer, SCOOP_STACKMAP_DIRECT, 8, 29, -16);
    write_location(&buffer, SCOOP_STACKMAP_DIRECT, 8, 29, -16);
    write_location(&buffer, SCOOP_STACKMAP_INDIRECT, 8, 31, 24);
    write_location(&buffer, SCOOP_STACKMAP_INDIRECT, 8, 31, 24);
    write_location(&buffer, SCOOP_STACKMAP_CONSTANT, 8, 0, 42);
    write_location(&buffer, SCOOP_STACKMAP_CONSTANT, 8, 0, 42);
    buffer.constant_index_location = buffer.length;
    write_location(&buffer, SCOOP_STACKMAP_CONSTANT_INDEX, 8, 0, 0);
    write_location(&buffer, SCOOP_STACKMAP_CONSTANT_INDEX, 8, 0, 0);
    write_record_end(&buffer, 0);
    return buffer;
}

static ScoopStackMapErrorCode parse(TestBuffer *buffer, size_t length,
                                    ScoopStackMapIndex *index) {
    ScoopStackMapImage image = {
        .section = buffer->bytes,
        .section_size = length,
        .text_start = 0x1000,
        .text_end = 0x2000,
    };
    ScoopStackMapError error;
    bool success = scoop_stackmap_build_index(&image, 1, index, &error);
    if (success) {
        assert(error.code == SCOOP_STACKMAP_OK);
        return SCOOP_STACKMAP_OK;
    }
    assert(index->records == NULL);
    assert(index->record_count == 0);
    return error.code;
}

static void expect_error(TestBuffer buffer, ScoopStackMapErrorCode expected) {
    ScoopStackMapIndex index;
    ScoopStackMapErrorCode actual = parse(&buffer, buffer.length, &index);
    assert(actual == expected);
}

static void set_u32(TestBuffer *buffer, size_t offset, uint32_t value) {
    assert(offset + 4 <= buffer->length);
    buffer->bytes[offset] = (uint8_t)value;
    buffer->bytes[offset + 1] = (uint8_t)(value >> 8);
    buffer->bytes[offset + 2] = (uint8_t)(value >> 16);
    buffer->bytes[offset + 3] = (uint8_t)(value >> 24);
}

static void set_u64(TestBuffer *buffer, size_t offset, uint64_t value) {
    set_u32(buffer, offset, (uint32_t)value);
    set_u32(buffer, offset + 4, (uint32_t)(value >> 32));
}

static void test_valid_section(void) {
    TestBuffer buffer = valid_section();
    ScoopStackMapIndex index;
    assert(parse(&buffer, buffer.length, &index) == SCOOP_STACKMAP_OK);
    assert(index.record_count == 2);
    assert(index.records[0].return_pc == 0x1010);
    assert(index.records[0].safepoint_id == 7);
    assert(index.records[0].stack_size == 64);
    assert(index.records[0].root_count == 5);
    assert(index.records[0].roots[0].base.kind ==
           SCOOP_STACKMAP_REGISTER);
    assert(index.records[0].roots[1].base.kind == SCOOP_STACKMAP_DIRECT);
    assert(index.records[0].roots[2].base.kind ==
           SCOOP_STACKMAP_INDIRECT);
    assert(index.records[0].roots[3].base.constant == 42);
    assert(index.records[0].roots[4].base.constant ==
           UINT64_C(0x1122334455667788));
    assert(scoop_stackmap_lookup(&index, 0x1010) == &index.records[0]);
    assert(scoop_stackmap_lookup(&index, 0x1020) == &index.records[1]);
    assert(scoop_stackmap_lookup(&index, 0x1014) == NULL);
    scoop_stackmap_dispose_index(&index);
}

static void test_every_truncation_is_rejected(void) {
    TestBuffer buffer = valid_section();
    for (size_t length = 0; length < buffer.length; length++) {
        ScoopStackMapIndex index;
        assert(parse(&buffer, length, &index) != SCOOP_STACKMAP_OK);
    }
}

static void test_corrupt_sections_are_rejected(void) {
    TestBuffer buffer = valid_section();
    buffer.bytes[0] = 2;
    expect_error(buffer, SCOOP_STACKMAP_UNSUPPORTED_VERSION);

    buffer = valid_section();
    buffer.bytes[1] = 1;
    expect_error(buffer, SCOOP_STACKMAP_NONZERO_RESERVED);

    buffer = valid_section();
    set_u64(&buffer, 24, 63);
    expect_error(buffer, SCOOP_STACKMAP_INVALID_STACK_SIZE);

    buffer = valid_section();
    set_u64(&buffer, 32, 3);
    expect_error(buffer, SCOOP_STACKMAP_INVALID_FUNCTION_COUNT);

    buffer = valid_section();
    set_u64(&buffer, 16, 0x0ff0);
    expect_error(buffer, SCOOP_STACKMAP_INVALID_FUNCTION_ADDRESS);

    buffer = valid_section();
    buffer.bytes[buffer.first_record + 16] = 9;
    expect_error(buffer, SCOOP_STACKMAP_INVALID_LOCATION);

    buffer = valid_section();
    set_u32(&buffer, buffer.first_record + 16 + 2 * 12 + 8, 1);
    expect_error(buffer, SCOOP_STACKMAP_DEOPT_UNSUPPORTED);

    buffer = valid_section();
    buffer.bytes[buffer.first_padding] = 1;
    expect_error(buffer, SCOOP_STACKMAP_NONZERO_RESERVED);

    buffer = valid_section();
    set_u64(&buffer, buffer.second_record, 9);
    expect_error(buffer, SCOOP_STACKMAP_DUPLICATE_SAFEPOINT_ID);

    buffer = valid_section();
    set_u32(&buffer, buffer.second_record + 8, 0x20);
    expect_error(buffer, SCOOP_STACKMAP_DUPLICATE_RETURN_PC);

    buffer = valid_section();
    set_u32(&buffer, buffer.constant_index_location + 8, 1);
    expect_error(buffer, SCOOP_STACKMAP_INVALID_CONSTANT_INDEX);

    buffer = valid_section();
    write_u8(&buffer, 0);
    expect_error(buffer, SCOOP_STACKMAP_TRAILING_DATA);
}

static void test_aarch64_stack_only_profile(void) {
    _Alignas(16) uintptr_t stack[16] = {0};
    uintptr_t sp = (uintptr_t)&stack[0];
    uintptr_t fp = sp + 48;
    uintptr_t boundary = sp + 96;
    stack[6] = boundary;
    stack[7] = 0x1234;
    ScoopStackMapLocation location = {
        .kind = SCOOP_STACKMAP_INDIRECT,
        .size = 8,
        .dwarf_register = 31,
        .offset = 16,
        .constant = 0,
    };
    ScoopStackMapRootPair root = {
        .base = location,
        .derived = location,
    };
    ScoopStackMapRecord record = {
        .safepoint_id = 17,
        .function_address = 0x1000,
        .return_pc = 0x1020,
        .stack_size = 64,
        .roots = &root,
        .root_count = 1,
    };
    ScoopPlatformError error = {0};
    assert(scoop_darwin_aarch64_managed_frame_ops.validate_record(&record,
                                                                  &error));

    ScoopManagedAnchor anchor = {
        .return_pc = record.return_pc,
        .stack_pointer = sp,
        .frame_pointer = fp,
        .previous = NULL,
    };
    ScoopPlatformStackBounds bounds = {
        .low = (const char *)sp,
        .high = (const char *)(sp + sizeof stack),
    };
    ScoopManagedFrame frame;
    assert(scoop_darwin_aarch64_managed_frame_ops.frame_from_anchor(
        &anchor, &record, bounds, &frame, &error));
    void **slot;
    assert(scoop_darwin_aarch64_managed_frame_ops.resolve_root(
        &frame, 0, &slot, &error));
    assert(slot == (void **)(sp + 16));

    uintptr_t next_pc;
    uintptr_t next_sp;
    uintptr_t next_fp;
    bool has_next = true;
    assert(scoop_darwin_aarch64_managed_frame_ops.next_frame(
        &frame, boundary, bounds, &next_pc, &next_sp, &next_fp, &has_next,
        &error));
    assert(!has_next);

    root.base.kind = SCOOP_STACKMAP_REGISTER;
    root.derived.kind = SCOOP_STACKMAP_REGISTER;
    error = (ScoopPlatformError){0};
    assert(!scoop_darwin_aarch64_managed_frame_ops.validate_record(&record,
                                                                   &error));
    assert(error.code == SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION);

    root.base = location;
    root.derived = location;
    root.derived.offset++;
    error = (ScoopPlatformError){0};
    assert(!scoop_darwin_aarch64_managed_frame_ops.validate_record(&record,
                                                                   &error));
    assert(error.code == SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION);

    root.derived = root.base;
    root.base.offset = 60;
    root.derived.offset = 60;
    error = (ScoopPlatformError){0};
    assert(!scoop_darwin_aarch64_managed_frame_ops.validate_record(&record,
                                                                   &error));
    assert(error.code == SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME);

    root.base = location;
    root.derived = location;
    anchor.frame_pointer -= 8;
    error = (ScoopPlatformError){0};
    assert(!scoop_darwin_aarch64_managed_frame_ops.frame_from_anchor(
        &anchor, &record, bounds, &frame, &error));
    assert(error.code == SCOOP_PLATFORM_INVALID_ANCHOR);
}

int main(void) {
    test_valid_section();
    test_every_truncation_is_rejected();
    test_corrupt_sections_are_rejected();
    test_aarch64_stack_only_profile();
    puts("stackmap parser tests passed");
    return 0;
}
