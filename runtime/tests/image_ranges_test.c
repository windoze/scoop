#include <assert.h>
#include <stdint.h>
#include <stdio.h>

#include "../src/platform/platform.h"

extern const ScoopMetadataImageOps scoop_macho_metadata_image_ops;

static uint64_t mutable_storage;
static const uint64_t *const descriptor
    __attribute__((used, section("__DATA_CONST,__const"))) = &mutable_storage;
static void managed_function(void) {}

static const uint64_t stackmap_words[]
    __attribute__((used, aligned(8), section("__LLVM_STACKMAPS,__llvm_stackmaps"))) = {
        (uint64_t)(uintptr_t)managed_function};

static void synthetic_ranges(void) {
    const ScoopPlatformImageRange ranges[] = {
        {0x1000, 0x2000, SCOOP_IMAGE_READ},
        {0x2000, 0x2800, SCOOP_IMAGE_READ},
        {0x3000, 0x4000, SCOOP_IMAGE_READ | SCOOP_IMAGE_WRITE},
        {0x4000, 0x5000, SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE},
    };
    ScoopPlatformMetadataImages images = {.ranges = ranges, .range_count = 4};
    assert(scoop_image_range_contains(&images, (void *)0x1800, 0x1000, 8,
                                      SCOOP_IMAGE_READ, SCOOP_IMAGE_WRITE));
    assert(!scoop_image_range_contains(&images, (void *)0x1800, 0x1001, 8,
                                       SCOOP_IMAGE_READ, SCOOP_IMAGE_WRITE));
    assert(!scoop_image_range_contains(&images, (void *)0x1801, 8, 8, SCOOP_IMAGE_READ,
                                       0));
    assert(!scoop_image_range_contains(&images, (void *)0x3000, 8, 8, SCOOP_IMAGE_READ,
                                       SCOOP_IMAGE_WRITE));
    assert(scoop_image_range_contains(&images, (void *)0x3000, 8, 8, SCOOP_IMAGE_WRITE,
                                      0));
    assert(scoop_image_range_contains(&images, (void *)0x4000, 4, 4,
                                      SCOOP_IMAGE_EXECUTE, 0));
    assert(!scoop_image_range_contains(&images, (void *)(UINTPTR_MAX - 7), 16, 8,
                                       SCOOP_IMAGE_READ, 0));
    assert(!scoop_image_range_contains(&images, NULL, 0, 8, SCOOP_IMAGE_READ, 0));
    assert(!scoop_image_range_contains(&images, (void *)0x1000, 8, 3, SCOOP_IMAGE_READ,
                                       0));
}

int main(void) {
    synthetic_ranges();
    ScoopPlatformMetadataImages images;
    ScoopPlatformError error = {0};
    assert(scoop_macho_metadata_image_ops.loaded_images(&images, &error));
    assert(images.range_count != 0 && images.count == 1);
    assert(scoop_image_range_contains(&images, &descriptor, sizeof descriptor,
                                      _Alignof(void *), SCOOP_IMAGE_READ,
                                      SCOOP_IMAGE_WRITE));
    assert(scoop_image_range_contains(&images, &mutable_storage, sizeof mutable_storage,
                                      _Alignof(uint64_t), SCOOP_IMAGE_WRITE,
                                      SCOOP_IMAGE_EXECUTE));
    assert(scoop_image_range_contains(&images, (const void *)main, 4, 4,
                                      SCOOP_IMAGE_EXECUTE, SCOOP_IMAGE_WRITE));
    assert(images.images[0].section == (const uint8_t *)stackmap_words);
    assert(images.images[0].section_size == sizeof stackmap_words);
    assert(stackmap_words[0] == (uint64_t)(uintptr_t)managed_function);
    for (size_t index = 1; index < images.range_count; index++) {
        assert(images.ranges[index - 1].end <= images.ranges[index].start);
    }
    scoop_macho_metadata_image_ops.dispose_images(&images);
    assert(images.ranges == NULL && images.images == NULL);
    puts("loaded image range tests passed");
}
