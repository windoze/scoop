#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include "../../src/platform/platform.h"

extern const ScoopThreadVmOps scoop_darwin_thread_vm_ops;
extern const ScoopManagedFrameOps scoop_darwin_aarch64_managed_frame_ops;

static uint8_t fake_stackmaps[104];
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

static void initialize_fake_stackmaps(void) {
    size_t cursor = 0;
    fake_stackmaps[cursor++] = 3;
    fake_stackmaps[cursor++] = 0;
    write_u16(&cursor, 0);
    write_u32(&cursor, 1);
    write_u32(&cursor, 0);
    write_u32(&cursor, 1);
    write_u64(&cursor, 0x1000);
    write_u64(&cursor, 16);
    write_u64(&cursor, 1);
    write_u64(&cursor, 1);
    write_u32(&cursor, 0x10);
    write_u16(&cursor, 0);
    write_u16(&cursor, 3);
    write_constant_location(&cursor);
    write_constant_location(&cursor);
    write_constant_location(&cursor);
    while (cursor % 8 != 0) {
        fake_stackmaps[cursor++] = 0;
    }
    write_u16(&cursor, 0);
    write_u16(&cursor, 0);
    while (cursor % 8 != 0) {
        fake_stackmaps[cursor++] = 0;
    }
    if (cursor != sizeof fake_stackmaps) {
        fprintf(stderr, "fake platform stackmap size drifted\n");
        abort();
    }
    fake_stackmaps_initialized = true;
}

static bool fake_loaded_images(ScoopPlatformMetadataImages *images,
                               ScoopPlatformError *error) {
    static ScoopStackMapImage image;
    if (images == NULL || error == NULL) {
        return false;
    }
    if (!fake_stackmaps_initialized) {
        initialize_fake_stackmaps();
    }
    image = (ScoopStackMapImage){
        .section = fake_stackmaps,
        .section_size = sizeof fake_stackmaps,
        .text_start = 0x1000,
        .text_end = 0x2000,
    };
    *images = (ScoopPlatformMetadataImages){
        .images = &image,
        .count = 1,
    };
    return true;
}

static const ScoopMetadataImageOps fake_metadata_ops = {
    .loaded_images = fake_loaded_images,
};

static const ScoopPlatformBundle fake_bundle = {
    .metadata_images = &fake_metadata_ops,
    .thread_vm = &scoop_darwin_thread_vm_ops,
    .managed_frames = &scoop_darwin_aarch64_managed_frame_ops,
};

const ScoopPlatformBundle *scoop_platform_bundle(void) {
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
    if (!fake_bundle.thread_vm->stack_bounds(&bounds, &error)) {
        fprintf(stderr, "fake platform: %s\n",
                scoop_platform_error_message(error.code));
        abort();
    }
    return bounds;
}
