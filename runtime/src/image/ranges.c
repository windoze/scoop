#include "../platform/platform.h"

bool scoop_image_range_contains(const ScoopPlatformMetadataImages *images,
                                const void *pointer, uint64_t size, uint64_t alignment,
                                uint32_t required, uint32_t forbidden) {
    uintptr_t start = (uintptr_t)pointer;
    if (images == NULL || pointer == NULL || alignment == 0 ||
        (alignment & (alignment - 1)) != 0 || start % alignment != 0 ||
        size > UINTPTR_MAX - start || images->ranges == NULL) {
        return false;
    }
    uintptr_t end = start + (uintptr_t)size;
    size_t low = 0;
    size_t high = images->range_count;
    while (low < high) {
        size_t middle = low + (high - low) / 2;
        if (images->ranges[middle].end <= start) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    for (size_t index = low; index < images->range_count; index++) {
        const ScoopPlatformImageRange *range = &images->ranges[index];
        if (range->start > start || (range->permissions & required) != required ||
            (range->permissions & forbidden) != 0) {
            return false;
        }
        if (end <= range->end) {
            return true;
        }
        start = range->end;
    }
    return false;
}
