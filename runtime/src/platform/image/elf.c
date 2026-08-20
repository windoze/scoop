#define _GNU_SOURCE
#include <inttypes.h>
#include <link.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "../platform.h"

#if !defined(__linux__) || UINTPTR_MAX != UINT64_MAX
#error "the ELF image component requires 64-bit Linux"
#endif

extern const uint8_t __scoop_stackmaps_start[]
    __attribute__((visibility("hidden")));
extern const uint8_t __scoop_stackmaps_end[]
    __attribute__((visibility("hidden")));

typedef struct ElfImageQuery {
    ScoopPlatformMetadataImages *images;
    ScoopPlatformImageRange *mappings;
    size_t mapping_count;
    ScoopStackMapImage stackmaps;
    bool found;
    bool valid;
} ElfImageQuery;

static bool elf_loaded_images(ScoopPlatformMetadataImages *images,
                              ScoopPlatformError *error);

static bool append_range(ScoopPlatformImageRange **ranges, size_t *count,
                         uintptr_t start, uintptr_t end, uint32_t permissions) {
    if (*count == SIZE_MAX / sizeof **ranges) {
        return false;
    }
    ScoopPlatformImageRange *next =
        realloc(*ranges, (*count + 1) * sizeof **ranges);
    if (next == NULL) {
        return false;
    }
    *ranges = next;
    next[(*count)++] = (ScoopPlatformImageRange){start, end, permissions};
    return true;
}

static bool read_mappings(ElfImageQuery *query) {
    FILE *file = fopen("/proc/self/maps", "r");
    if (file == NULL) {
        return false;
    }
    char *line = NULL;
    size_t capacity = 0;
    bool valid = true;
    while (getline(&line, &capacity, file) != -1) {
        uintptr_t start, end;
        char permissions[5];
        if (sscanf(line, "%" SCNxPTR "-%" SCNxPTR " %4s", &start, &end,
                   permissions) != 3 ||
            strlen(permissions) != 4 || start >= end ||
            (query->mapping_count != 0 &&
             query->mappings[query->mapping_count - 1].end > start)) {
            valid = false;
            break;
        }
        uint32_t flags = (permissions[0] == 'r' ? SCOOP_IMAGE_READ : 0) |
                         (permissions[1] == 'w' ? SCOOP_IMAGE_WRITE : 0) |
                         (permissions[2] == 'x' ? SCOOP_IMAGE_EXECUTE : 0);
        if (!append_range(&query->mappings, &query->mapping_count, start, end,
                          flags)) {
            valid = false;
            break;
        }
    }
    valid = valid && feof(file) && query->mapping_count != 0;
    free(line);
    fclose(file);
    return valid;
}

static bool loaded_range(const struct dl_phdr_info *info,
                         const ElfW(Phdr) * header, uintptr_t *start,
                         uintptr_t *end) {
    if (header->p_vaddr > UINTPTR_MAX - info->dlpi_addr) {
        return false;
    }
    *start = (uintptr_t)(info->dlpi_addr + header->p_vaddr);
    if (header->p_memsz > UINTPTR_MAX - *start) {
        return false;
    }
    *end = *start + (uintptr_t)header->p_memsz;
    return true;
}

static bool append_segment(ElfImageQuery *query, uintptr_t cursor,
                           uintptr_t limit) {
    ScoopPlatformMetadataImages *images = query->images;
    for (size_t index = 0; index < query->mapping_count && cursor < limit;
         ++index) {
        const ScoopPlatformImageRange *mapping = &query->mappings[index];
        if (mapping->end <= cursor) {
            continue;
        }
        if (mapping->start > cursor) {
            return false;
        }
        uintptr_t end = mapping->end < limit ? mapping->end : limit;
        ScoopPlatformImageRange *ranges = (void *)images->ranges;
        bool ok = append_range(&ranges, &images->range_count, cursor, end,
                               mapping->permissions);
        images->ranges = ranges;
        if (!ok) {
            return false;
        }
        cursor = end;
    }
    return cursor == limit;
}

static int collect_image(struct dl_phdr_info *info, size_t size, void *data) {
    (void)size;
    ElfImageQuery *query = data;
    uintptr_t own_code = (uintptr_t)elf_loaded_images;
    bool contains_runtime = false;
    for (ElfW(Half) index = 0; index < info->dlpi_phnum; ++index) {
        const ElfW(Phdr) *header = &info->dlpi_phdr[index];
        uintptr_t start, end;
        if (header->p_type == PT_LOAD &&
            loaded_range(info, header, &start, &end) && own_code >= start &&
            own_code < end) {
            contains_runtime = true;
            break;
        }
    }
    if (!contains_runtime) {
        return 0;
    }
    query->found = true;
    for (ElfW(Half) index = 0; index < info->dlpi_phnum; ++index) {
        const ElfW(Phdr) *header = &info->dlpi_phdr[index];
        if (header->p_type != PT_LOAD || header->p_memsz == 0) {
            continue;
        }
        uintptr_t start, end;
        if (!loaded_range(info, header, &start, &end) ||
            !append_segment(query, start, end)) {
            query->valid = false;
            return 1;
        }
        if (header->p_flags & PF_X) {
            if (query->stackmaps.text_start == 0 ||
                start < query->stackmaps.text_start) {
                query->stackmaps.text_start = start;
            }
            if (end > query->stackmaps.text_end) {
                query->stackmaps.text_end = end;
            }
        }
    }
    return 1;
}

static int compare_range(const void *left, const void *right) {
    uintptr_t a = ((const ScoopPlatformImageRange *)left)->start;
    uintptr_t b = ((const ScoopPlatformImageRange *)right)->start;
    return (a > b) - (a < b);
}

static void elf_dispose_images(ScoopPlatformMetadataImages *images) {
    free((void *)images->ranges);
    free((void *)images->images);
    *images = (ScoopPlatformMetadataImages){0};
}

static bool collect(ScoopPlatformMetadataImages *images) {
    ElfImageQuery query = {.images = images, .valid = true};
    if (!read_mappings(&query)) {
        free(query.mappings);
        return false;
    }
    dl_iterate_phdr(collect_image, &query);
    free(query.mappings);
    if (!query.found || !query.valid || images->range_count == 0) {
        return false;
    }
    qsort((void *)images->ranges, images->range_count, sizeof *images->ranges,
          compare_range);
    for (size_t index = 1; index < images->range_count; ++index) {
        if (images->ranges[index - 1].end > images->ranges[index].start) {
            return false;
        }
    }
    ScoopStackMapImage image = query.stackmaps;
    if (image.text_start == 0 || image.text_end <= image.text_start ||
        !scoop_image_range_contains(images, (const void *)image.text_start,
                                    image.text_end - image.text_start, 1,
                                    SCOOP_IMAGE_READ | SCOOP_IMAGE_EXECUTE,
                                    SCOOP_IMAGE_WRITE)) {
        return false;
    }
    uintptr_t start = (uintptr_t)__scoop_stackmaps_start;
    uintptr_t end = (uintptr_t)__scoop_stackmaps_end;
    if (start > end) {
        return false;
    }
    if (start == end) {
        return true;
    }
    image.section = (const uint8_t *)start;
    image.section_size = end - start;
    if (!scoop_image_range_contains(images, image.section, image.section_size,
                                    8, SCOOP_IMAGE_READ, SCOOP_IMAGE_WRITE)) {
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

static bool elf_loaded_images(ScoopPlatformMetadataImages *images,
                              ScoopPlatformError *error) {
    if (images == NULL || error == NULL) {
        return false;
    }
    *images = (ScoopPlatformMetadataImages){0};
    if (!collect(images)) {
        elf_dispose_images(images);
        error->code = SCOOP_PLATFORM_METADATA_UNAVAILABLE;
        return false;
    }
    *error = (ScoopPlatformError){0};
    return true;
}

const ScoopMetadataImageOps scoop_elf_metadata_image_ops = {
    .loaded_images = elf_loaded_images,
    .dispose_images = elf_dispose_images,
};
