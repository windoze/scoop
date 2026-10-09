#define _GNU_SOURCE
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <unistd.h>

#include "../platform.h"

#if !defined(__linux__)
#error "the Linux OS component requires Linux"
#endif

static bool linux_stack_bounds(ScoopPlatformStackBounds *bounds, ScoopPlatformError *error) {
    if (bounds == NULL || error == NULL) {
        return false;
    }
    pthread_attr_t attributes;
    if (pthread_getattr_np(pthread_self(), &attributes) != 0) {
        error->code = SCOOP_PLATFORM_INVALID_STACK_BOUNDS;
        return false;
    }
    void *base = NULL;
    size_t size = 0;
    int result = pthread_attr_getstack(&attributes, &base, &size);
    pthread_attr_destroy(&attributes);
    uintptr_t start = (uintptr_t)base;
    if (result != 0 || start == 0 || size == 0 || size > UINTPTR_MAX - start) {
        error->code = SCOOP_PLATFORM_INVALID_STACK_BOUNDS;
        return false;
    }
    *bounds = (ScoopPlatformStackBounds){
        .low = base,
        .high = (const char *)(start + size),
    };
    return true;
}

static size_t linux_page_size(void) {
    long size = sysconf(_SC_PAGESIZE);
    return size > 0 ? (size_t)size : 0;
}

static bool linux_reserve_read_write(uintptr_t preferred_address, size_t size, void **mapping,
                                     ScoopPlatformError *error) {
    if (size == 0 || mapping == NULL || error == NULL) {
        return false;
    }
    void *reserved = mmap((void *)preferred_address, size, PROT_READ | PROT_WRITE,
                          MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
    if (reserved == MAP_FAILED) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    *mapping = reserved;
    return true;
}

static bool linux_protect_none(void *base, size_t size, ScoopPlatformError *error) {
    size_t page_size = linux_page_size();
    if (error == NULL) {
        return false;
    }
    if (base == NULL || page_size == 0 || size == 0 || (uintptr_t)base % page_size != 0 ||
        size % page_size != 0 || mprotect(base, size, PROT_NONE) != 0) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static bool linux_release_mapping(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL) {
        return false;
    }
    if (base == NULL || size == 0 || munmap(base, size) != 0) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static bool linux_discard_pages(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL) {
        return false;
    }
    if (base == NULL || size == 0 || madvise(base, size, MADV_DONTNEED) != 0) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static void linux_resident_memory(uint64_t *current_bytes, uint64_t *peak_bytes) {
    *current_bytes = *peak_bytes = 0;
    FILE *statm = fopen("/proc/self/statm", "r");
    if (statm != NULL) {
        unsigned long virtual_pages, resident_pages;
        if (fscanf(statm, "%lu %lu", &virtual_pages, &resident_pages) == 2) {
            *current_bytes = (uint64_t)resident_pages * linux_page_size();
        }
        fclose(statm);
    }
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage) == 0 && usage.ru_maxrss > 0) {
        *peak_bytes = (uint64_t)usage.ru_maxrss * 1024;
    }
}

const ScoopThreadVmOps scoop_linux_thread_vm_ops = {
    .stack_bounds = linux_stack_bounds,
    .reserve_read_write = linux_reserve_read_write,
    .page_size = linux_page_size,
    .protect_none = linux_protect_none,
    .release_mapping = linux_release_mapping,
    .discard_pages = linux_discard_pages,
    .resident_memory = linux_resident_memory,
};
