#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <unistd.h>

#include "../src/platform/platform.h"

extern const ScoopMetadataImageOps scoop_elf_metadata_image_ops;

static uint64_t mutable_storage;
static const uint64_t *const descriptor
    __attribute__((used, section(".data.rel.ro.scoop.test"))) =
        &mutable_storage;
static void managed_function(void) {}
static const uintptr_t stackmap_words[]
    __attribute__((used, aligned(8), section(".llvm_stackmaps"))) = {
        (uintptr_t)managed_function};

int main(void) {
    assert(scoop_platform_bundle()->metadata_images ==
           &scoop_elf_metadata_image_ops);
    ScoopPlatformStackBounds stack = scoop_platform_stack_bounds();
    assert((uintptr_t)&stack >= (uintptr_t)stack.low &&
           (uintptr_t)&stack < (uintptr_t)stack.high);
    ScoopPlatformMetadataImages images;
    ScoopPlatformError error = {0};
    assert(scoop_elf_metadata_image_ops.loaded_images(&images, &error));
    assert(images.range_count != 0 && images.count == 1);
    assert(scoop_image_range_contains(&images, &descriptor, sizeof descriptor,
                                      _Alignof(void *), SCOOP_IMAGE_READ,
                                      SCOOP_IMAGE_WRITE));
    assert(scoop_image_range_contains(
        &images, &mutable_storage, sizeof mutable_storage, _Alignof(uint64_t),
        SCOOP_IMAGE_WRITE, SCOOP_IMAGE_EXECUTE));
    assert(scoop_image_range_contains(&images, (const void *)main, 1, 1,
                                      SCOOP_IMAGE_EXECUTE, SCOOP_IMAGE_WRITE));
    assert(images.images[0].section == (const uint8_t *)stackmap_words);
    assert(images.images[0].section_size == sizeof stackmap_words);
    assert(stackmap_words[0] == (uintptr_t)managed_function);
    for (size_t index = 1; index < images.range_count; ++index) {
        assert(images.ranges[index - 1].end <= images.ranges[index].start);
    }
    scoop_elf_metadata_image_ops.dispose_images(&images);
    assert(images.ranges == NULL && images.images == NULL);

    /* Prove that the next snapshot observes actual permissions, not ELF flags.
     */
    long page_size = sysconf(_SC_PAGESIZE);
    assert(page_size > 0);
    uintptr_t page =
        (uintptr_t)stackmap_words - (uintptr_t)stackmap_words % page_size;
    assert(mprotect((void *)page, (size_t)page_size, PROT_READ | PROT_WRITE) ==
           0);
    assert(!scoop_elf_metadata_image_ops.loaded_images(&images, &error));
    assert(error.code == SCOOP_PLATFORM_METADATA_UNAVAILABLE);
    assert(images.ranges == NULL && images.images == NULL);
    assert(mprotect((void *)page, (size_t)page_size, PROT_READ) == 0);
    assert(scoop_elf_metadata_image_ops.loaded_images(&images, &error));
    scoop_elf_metadata_image_ops.dispose_images(&images);
    puts("ELF load bias, stackmap bounds and actual metadata permissions "
         "passed");
}
