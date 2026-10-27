#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include "../platform.h"
#include <stdlib.h>
#include <string.h>

#if !defined(_WIN64)
#error "the PE image component requires 64-bit Windows"
#endif

static bool pe_loaded_images(ScoopPlatformMetadataImages *images, ScoopPlatformError *error);

static int compare_range(const void *left, const void *right) {
    uintptr_t a = ((const ScoopPlatformImageRange *)left)->start;
    uintptr_t b = ((const ScoopPlatformImageRange *)right)->start;
    return (a > b) - (a < b);
}

static void pe_dispose_images(ScoopPlatformMetadataImages *images) {
    free((void *)images->ranges);
    free((void *)images->images);
    *images = (ScoopPlatformMetadataImages){0};
}

static bool collect(ScoopPlatformMetadataImages *images) {
    HMODULE module;
    if (!GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                            (LPCWSTR)(uintptr_t)pe_loaded_images, &module))
        return false;
    const uint8_t *base = (const uint8_t *)module;
    const IMAGE_DOS_HEADER *dos = (const void *)base;
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew < (LONG)sizeof *dos)
        return false;
    const IMAGE_NT_HEADERS64 *pe = (const void *)(base + dos->e_lfanew);
    if (pe->Signature != IMAGE_NT_SIGNATURE || pe->FileHeader.Machine != IMAGE_FILE_MACHINE_AMD64 ||
        pe->FileHeader.SizeOfOptionalHeader < sizeof pe->OptionalHeader ||
        pe->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC)
        return false;
    size_t image_size = pe->OptionalHeader.SizeOfImage;
    size_t header_size = pe->OptionalHeader.SizeOfHeaders;
    size_t table_offset = (size_t)dos->e_lfanew + offsetof(IMAGE_NT_HEADERS64, OptionalHeader) +
                          pe->FileHeader.SizeOfOptionalHeader;
    size_t count = pe->FileHeader.NumberOfSections;
    if (count == 0 || image_size > UINTPTR_MAX - (uintptr_t)base || header_size > image_size ||
        table_offset > header_size ||
        count > (header_size - table_offset) / sizeof(IMAGE_SECTION_HEADER))
        return false;
    const IMAGE_SECTION_HEADER *sections = (const void *)(base + table_offset);
    ScoopPlatformImageRange *ranges = calloc(count, sizeof *ranges);
    if (ranges == NULL)
        return false;
    images->ranges = ranges;
    ScoopStackMapImage image = {0};
    for (size_t index = 0; index < count; ++index) {
        const IMAGE_SECTION_HEADER *section = &sections[index];
        size_t offset = section->VirtualAddress, size = section->Misc.VirtualSize;
        if (size == 0 || (section->Characteristics & IMAGE_SCN_MEM_DISCARDABLE) != 0)
            continue;
        if (offset > image_size || size > image_size - offset)
            return false;
        uintptr_t start = (uintptr_t)base + offset;
        ranges[images->range_count++] = (ScoopPlatformImageRange){start, start + size};
        if (memcmp(section->Name, ".text\0\0\0", IMAGE_SIZEOF_SHORT_NAME) == 0) {
            if (image.text_start != 0)
                return false;
            image.text_start = start;
            image.text_end = start + size;
        } else if (memcmp(section->Name, ".smap\0\0\0", IMAGE_SIZEOF_SHORT_NAME) == 0) {
            if (image.section != NULL)
                return false;
            image.section = (const void *)start;
            image.section_size = size;
        }
    }
    qsort(ranges, images->range_count, sizeof *ranges, compare_range);
    for (size_t index = 1; index < images->range_count; ++index) {
        if (ranges[index - 1].end > ranges[index].start)
            return false;
    }
    if (image.text_start == 0 || !scoop_image_range_contains(images, (const void *)image.text_start,
                                                             image.text_end - image.text_start, 1))
        return false;
    if (image.section_size == 0)
        return true;
    if (!scoop_image_range_contains(images, image.section, image.section_size, 8))
        return false;
    ScoopStackMapImage *owned = malloc(sizeof *owned);
    if (owned == NULL)
        return false;
    *owned = image;
    images->images = owned;
    images->count = 1;
    return true;
}

static bool pe_loaded_images(ScoopPlatformMetadataImages *images, ScoopPlatformError *error) {
    if (images == NULL || error == NULL)
        return false;
    *images = (ScoopPlatformMetadataImages){0};
    if (!collect(images)) {
        pe_dispose_images(images);
        error->code = SCOOP_PLATFORM_METADATA_UNAVAILABLE;
        return false;
    }
    *error = (ScoopPlatformError){0};
    return true;
}

const ScoopMetadataImageOps scoop_pe_metadata_image_ops = {
    .loaded_images = pe_loaded_images,
    .dispose_images = pe_dispose_images,
};
