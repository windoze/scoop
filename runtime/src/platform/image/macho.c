#include <mach-o/dyld.h>
#include <mach-o/loader.h>
#include <mach/mach.h>
#include <mach/mach_vm.h>
#include <stdlib.h>
#include <string.h>

#include "../platform.h"

#if !defined(__APPLE__) || !defined(__LP64__)
#error "the Mach-O image component requires 64-bit Darwin"
#endif

static bool region_at(uintptr_t cursor, uintptr_t *end, uint32_t *permissions) {
    mach_vm_address_t address = cursor;
    mach_vm_size_t size = 0;
    vm_region_basic_info_data_64_t info;
    mach_msg_type_number_t count = VM_REGION_BASIC_INFO_COUNT_64;
    mach_port_t object = MACH_PORT_NULL;
    kern_return_t result =
        mach_vm_region(mach_task_self(), &address, &size, VM_REGION_BASIC_INFO_64,
                       (vm_region_info_t)&info, &count, &object);
    if (MACH_PORT_VALID(object)) {
        mach_port_deallocate(mach_task_self(), object);
    }
    if (result != KERN_SUCCESS || address > cursor || size == 0 ||
        size > UINTPTR_MAX - address || address + size <= cursor) {
        return false;
    }
    *end = (uintptr_t)(address + size);
    *permissions = ((info.protection & VM_PROT_READ) ? SCOOP_IMAGE_READ : 0) |
                   ((info.protection & VM_PROT_WRITE) ? SCOOP_IMAGE_WRITE : 0) |
                   ((info.protection & VM_PROT_EXECUTE) ? SCOOP_IMAGE_EXECUTE : 0);
    return true;
}

static bool readable(uintptr_t address, uint64_t size) {
    if (address == 0 || size > UINTPTR_MAX - address) {
        return false;
    }
    uintptr_t limit = address + (uintptr_t)size;
    while (address < limit) {
        uintptr_t end;
        uint32_t permissions;
        if (!region_at(address, &end, &permissions) ||
            !(permissions & SCOOP_IMAGE_READ)) {
            return false;
        }
        address = end;
    }
    return true;
}

static bool slid_address(uint64_t address, intptr_t slide, uintptr_t *result) {
    uint64_t amount = slide < 0 ? (uint64_t)(-(slide + 1)) + 1 : (uint64_t)slide;
    if ((slide < 0 && address < amount) ||
        (slide >= 0 && address > UINTPTR_MAX - amount)) {
        return false;
    }
    *result = (uintptr_t)(slide < 0 ? address - amount : address + amount);
    return true;
}

static bool append_segment(ScoopPlatformMetadataImages *images,
                           const struct segment_command_64 *segment, intptr_t slide) {
    if (segment->vmsize == 0 ||
        (segment->initprot == VM_PROT_NONE && segment->maxprot == VM_PROT_NONE)) {
        return true;
    }
    uintptr_t cursor;
    if (!slid_address(segment->vmaddr, slide, &cursor) ||
        segment->vmsize > UINTPTR_MAX - cursor) {
        return false;
    }
    uintptr_t limit = cursor + (uintptr_t)segment->vmsize;
    while (cursor < limit) {
        size_t length = images->range_count;
        uintptr_t end;
        uint32_t permissions;
        if (!region_at(cursor, &end, &permissions) ||
            length == SIZE_MAX / sizeof(ScoopPlatformImageRange)) {
            return false;
        }
        if (end > limit) {
            end = limit;
        }
        ScoopPlatformImageRange *ranges =
            realloc((void *)images->ranges, (length + 1) * sizeof *ranges);
        if (ranges == NULL) {
            return false;
        }
        images->ranges = ranges;
        ranges[images->range_count++] =
            (ScoopPlatformImageRange){cursor, end, permissions};
        cursor = end;
    }
    return true;
}

static bool read_sections(const struct segment_command_64 *segment, intptr_t slide,
                          ScoopStackMapImage *image) {
    uint64_t expected =
        sizeof *segment + (uint64_t)segment->nsects * sizeof(struct section_64);
    if (expected != segment->cmdsize) {
        return false;
    }
    const struct section_64 *sections = (const struct section_64 *)(segment + 1);
    for (uint32_t index = 0; index < segment->nsects; index++) {
        const struct section_64 *section = &sections[index];
        bool stackmaps = strncmp(section->segname, "__DATA_CONST", 16) == 0 &&
                         strncmp(section->sectname, "__llvm_stackmaps", 16) == 0;
        bool text = strncmp(section->segname, "__TEXT", 16) == 0 &&
                    strncmp(section->sectname, "__text", 16) == 0;
        if (!stackmaps && !text) {
            continue;
        }
        uintptr_t address;
        if (!slid_address(section->addr, slide, &address) ||
            section->size > UINTPTR_MAX - address) {
            return false;
        }
        if (stackmaps) {
            if (image->section != NULL || section->size > SIZE_MAX) {
                return false;
            }
            image->section = (const uint8_t *)address;
            image->section_size = (size_t)section->size;
        } else {
            if (image->text_start != 0) {
                return false;
            }
            image->text_start = address;
            image->text_end = address + (uintptr_t)section->size;
        }
    }
    return true;
}

static int compare_range(const void *left, const void *right) {
    uintptr_t a = ((const ScoopPlatformImageRange *)left)->start;
    uintptr_t b = ((const ScoopPlatformImageRange *)right)->start;
    return (a > b) - (a < b);
}

static void macho_dispose_images(ScoopPlatformMetadataImages *images) {
    free((void *)images->ranges);
    free((void *)images->images);
    *images = (ScoopPlatformMetadataImages){0};
}

static bool collect(ScoopPlatformMetadataImages *images) {
    const struct mach_header_64 *header =
        (const struct mach_header_64 *)_dyld_get_image_header(0);
    if (!readable((uintptr_t)header, sizeof *header) || header->magic != MH_MAGIC_64 ||
        header->filetype != MH_EXECUTE || header->reserved != 0 ||
        header->ncmds > header->sizeofcmds / sizeof(struct load_command)) {
        return false;
    }
    uintptr_t cursor = (uintptr_t)(header + 1);
    if (!readable(cursor, header->sizeofcmds)) {
        return false;
    }
    uintptr_t limit = cursor + header->sizeofcmds;
    intptr_t slide = _dyld_get_image_vmaddr_slide(0);
    ScoopStackMapImage image = {0};
    for (uint32_t index = 0; index < header->ncmds; index++) {
        if (limit - cursor < sizeof(struct load_command)) {
            return false;
        }
        const struct load_command *command = (const struct load_command *)cursor;
        if (command->cmdsize < sizeof *command || command->cmdsize % 8 != 0 ||
            command->cmdsize > limit - cursor) {
            return false;
        }
        if (command->cmd == LC_SEGMENT_64) {
            const struct segment_command_64 *segment = (const void *)command;
            if (command->cmdsize < sizeof *segment ||
                !append_segment(images, segment, slide) ||
                !read_sections(segment, slide, &image)) {
                return false;
            }
        }
        cursor += command->cmdsize;
    }
    if (cursor != limit || images->range_count == 0) {
        return false;
    }
    qsort((void *)images->ranges, images->range_count, sizeof *images->ranges,
          compare_range);
    for (size_t index = 1; index < images->range_count; index++) {
        if (images->ranges[index - 1].end > images->ranges[index].start) {
            return false;
        }
    }
    if (!scoop_image_range_contains(
            images, (const void *)image.text_start, image.text_end - image.text_start,
            1, SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE, SCOOP_IMAGE_WRITE)) {
        return false;
    }
    if (image.section_size == 0) {
        return true;
    }
    if (!scoop_image_range_contains(images, image.section, image.section_size, 8,
                                    SCOOP_IMAGE_READ, SCOOP_IMAGE_WRITE)) {
        return false;
    }
    ScoopStackMapImage *owned = malloc(sizeof *owned);
    if (owned == NULL) {
        return false;
    }
    *owned = image;
    images->images = owned;
    images->count = 1;
    return true;
}

static bool macho_loaded_images(ScoopPlatformMetadataImages *images,
                                ScoopPlatformError *error) {
    if (images == NULL || error == NULL) {
        return false;
    }
    *images = (ScoopPlatformMetadataImages){0};
    if (!collect(images)) {
        macho_dispose_images(images);
        error->code = SCOOP_PLATFORM_METADATA_UNAVAILABLE;
        return false;
    }
    *error = (ScoopPlatformError){0};
    return true;
}

const ScoopMetadataImageOps scoop_macho_metadata_image_ops = {
    .loaded_images = macho_loaded_images,
    .dispose_images = macho_dispose_images,
};
