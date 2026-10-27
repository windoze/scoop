#ifndef SCOOP_POSIX_MAPPING_H
#define SCOOP_POSIX_MAPPING_H

#include "../platform.h"
#include <sys/mman.h>

static bool scoop_posix_reserve_read_write(size_t size, size_t alignment, size_t page,
                                           void **mapping, ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    if (mapping == NULL || page == 0 || size == 0 || size % page != 0 || alignment < page ||
        alignment % page != 0 || (alignment & (alignment - 1)) != 0 ||
        size > SIZE_MAX - (alignment - page)) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    size_t reserved_size = size + (alignment - page);
    void *reserved =
        mmap(NULL, reserved_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if (reserved == MAP_FAILED) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    uintptr_t raw = (uintptr_t)reserved;
    uintptr_t aligned = (raw + alignment - 1) & ~(uintptr_t)(alignment - 1);
    size_t prefix = aligned - raw;
    size_t suffix = reserved_size - prefix - size;
    if (prefix != 0 && munmap(reserved, prefix) != 0) {
        munmap(reserved, reserved_size);
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    if (suffix != 0 && munmap((void *)(aligned + size), suffix) != 0) {
        munmap((void *)aligned, size + suffix);
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    *mapping = (void *)aligned;
    return true;
}

#endif
