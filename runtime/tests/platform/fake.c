#include <stdbool.h>
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "../../src/platform/platform.h"

#if defined(__linux__)
extern const ScoopThreadVmOps scoop_linux_thread_vm_ops;
#define TEST_THREAD_VM_OPS scoop_linux_thread_vm_ops
#else
extern const ScoopThreadVmOps scoop_darwin_thread_vm_ops;
#define TEST_THREAD_VM_OPS scoop_darwin_thread_vm_ops
#endif
/* The fixture uses synthetic AArch64 frames on every host. Its OS component
 * operates on real host threads and mappings. */
extern const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops;

static uint8_t fake_stackmaps[160];
static size_t fake_stackmaps_size;
static bool fake_stackmaps_initialized;

static void write_u16(size_t *cursor, uint16_t value) {
    fake_stackmaps[(*cursor)++] = (uint8_t)value;
    fake_stackmaps[(*cursor)++] = (uint8_t)(value >> 8);
}

static void write_u32(size_t *cursor, uint32_t value) {
    write_u16(cursor, (uint16_t)value);
    write_u16(cursor, (uint16_t)(value >> 16));
}

static void write_u64(size_t *cursor, uint64_t value) {
    write_u32(cursor, (uint32_t)value);
    write_u32(cursor, (uint32_t)(value >> 32));
}

static void write_constant_location(size_t *cursor) {
    fake_stackmaps[(*cursor)++] = SCOOP_STACKMAP_CONSTANT;
    fake_stackmaps[(*cursor)++] = 0;
    write_u16(cursor, 8);
    write_u16(cursor, 0);
    write_u16(cursor, 0);
    write_u32(cursor, 0);
}

static void write_indirect_location(size_t *cursor, uint16_t dwarf_register, int32_t offset) {
    fake_stackmaps[(*cursor)++] = SCOOP_STACKMAP_INDIRECT;
    fake_stackmaps[(*cursor)++] = 0;
    write_u16(cursor, 8);
    write_u16(cursor, dwarf_register);
    write_u16(cursor, 0);
    write_u32(cursor, (uint32_t)offset);
}

static void initialize_fake_stackmaps(void) {
    size_t cursor = 0;
    fake_stackmaps[cursor++] = 3;
    fake_stackmaps[cursor++] = 0;
    write_u16(&cursor, 0);
    write_u32(&cursor, 1);
    write_u32(&cursor, 0);
    write_u32(&cursor, 1);
    write_u64(&cursor, 0x1000);
    write_u64(&cursor, 32);
    write_u64(&cursor, 1);
    write_u64(&cursor, 1);
    write_u32(&cursor, 0x10);
    write_u16(&cursor, 0);
    write_u16(&cursor, 5);
    write_constant_location(&cursor);
    write_constant_location(&cursor);
    write_constant_location(&cursor);
    write_indirect_location(&cursor, 31, 0);
    write_indirect_location(&cursor, 31, 0);
    while (cursor % 8 != 0) {
        fake_stackmaps[cursor++] = 0;
    }
    write_u16(&cursor, 0);
    write_u16(&cursor, 0);
    while (cursor % 8 != 0) {
        fake_stackmaps[cursor++] = 0;
    }
    if (cursor > sizeof fake_stackmaps) {
        fprintf(stderr, "fake platform stackmap size drifted\n");
        abort();
    }
    fake_stackmaps_size = cursor;
    fake_stackmaps_initialized = true;
}

static bool fake_loaded_images(ScoopPlatformMetadataImages *images, ScoopPlatformError *error) {
    static ScoopStackMapImage image;
    if (images == NULL || error == NULL) {
        return false;
    }
    if (!fake_stackmaps_initialized) {
        initialize_fake_stackmaps();
    }
    image = (ScoopStackMapImage){
        .section = fake_stackmaps,
        .section_size = fake_stackmaps_size,
        .text_start = 0x1000,
        .text_end = 0x2000,
    };
    *images = (ScoopPlatformMetadataImages){
        .images = &image,
        .count = 1,
    };
    return true;
}

static void fake_dispose_images(ScoopPlatformMetadataImages *images) {
    *images = (ScoopPlatformMetadataImages){0};
}

static const ScoopMetadataImageOps fake_metadata_ops = {
    .loaded_images = fake_loaded_images,
    .dispose_images = fake_dispose_images,
};

bool scoop_test_vm_fail_mappings;
bool scoop_test_vm_fail_discards;
void (*scoop_test_vm_discard_observer)(void *base, size_t size);
static ScoopThreadVmOps fake_vm_ops;
static pthread_once_t fake_vm_once = PTHREAD_ONCE_INIT;

static bool fake_reserve(uintptr_t address, size_t size, void **mapping,
                         ScoopPlatformError *error) {
    if (scoop_test_vm_fail_mappings) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return TEST_THREAD_VM_OPS.reserve_read_write(address, size, mapping, error);
}

static bool fake_discard(void *base, size_t size, ScoopPlatformError *error) {
    if (scoop_test_vm_discard_observer != NULL) {
        scoop_test_vm_discard_observer(base, size);
    }
    if (scoop_test_vm_fail_discards) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return TEST_THREAD_VM_OPS.discard_pages(base, size, error);
}

static void initialize_fake_vm(void) {
    fake_vm_ops = TEST_THREAD_VM_OPS;
    fake_vm_ops.reserve_read_write = fake_reserve;
    fake_vm_ops.discard_pages = fake_discard;
}

static const ScoopPlatformBundle fake_bundle = {
    .metadata_images = &fake_metadata_ops,
    .thread_vm = &fake_vm_ops,
    .managed_frames = &scoop_darwin_aarch64_managed_frame_ops,
};

const ScoopPlatformBundle *scoop_platform_bundle(void) {
    if (pthread_once(&fake_vm_once, initialize_fake_vm) != 0) {
        abort();
    }
    return &fake_bundle;
}

const char *scoop_platform_error_message(ScoopPlatformErrorCode code) {
    switch (code) {
    case SCOOP_PLATFORM_OK:
        return "no error";
    case SCOOP_PLATFORM_METADATA_UNAVAILABLE:
        return "fake image metadata is unavailable";
    case SCOOP_PLATFORM_INVALID_STACK_BOUNDS:
        return "fake thread stack bounds are invalid";
    case SCOOP_PLATFORM_INVALID_ANCHOR:
        return "fake managed anchor is invalid";
    case SCOOP_PLATFORM_INVALID_FRAME:
        return "fake managed frame is invalid";
    case SCOOP_PLATFORM_UNSUPPORTED_ROOT_LOCATION:
        return "fake root location is unsupported";
    case SCOOP_PLATFORM_ROOT_OUTSIDE_FRAME:
        return "fake root lies outside its frame";
    case SCOOP_PLATFORM_VM_OPERATION_FAILED:
        return "fake VM operation failed";
    }
    return "unknown fake platform error";
}

ScoopPlatformStackBounds scoop_platform_stack_bounds(void) {
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    if (!scoop_platform_bundle()->thread_vm->stack_bounds(&bounds, &error)) {
        fprintf(stderr, "fake platform: %s\n", scoop_platform_error_message(error.code));
        abort();
    }
    return bounds;
}
