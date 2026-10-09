#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE
#endif
#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <mach/mach.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <unistd.h>

#include "../platform.h"

#if !defined(__APPLE__) || !defined(__aarch64__)
#error "the Darwin OS component requires macOS on AArch64"
#endif

static bool darwin_stack_bounds(ScoopPlatformStackBounds *bounds, ScoopPlatformError *error) {
    if (bounds == NULL || error == NULL) {
        return false;
    }
    pthread_t self = pthread_self();
    void *stack_high = pthread_get_stackaddr_np(self);
    size_t stack_size = pthread_get_stacksize_np(self);
    if (stack_high == NULL || stack_size == 0 || stack_size > (size_t)(uintptr_t)stack_high) {
        error->code = SCOOP_PLATFORM_INVALID_STACK_BOUNDS;
        return false;
    }
    *bounds = (ScoopPlatformStackBounds){
        .low = (const char *)stack_high - stack_size,
        .high = stack_high,
    };
    return true;
}

static size_t darwin_page_size(void) {
    long size = sysconf(_SC_PAGESIZE);
    return size > 0 ? (size_t)size : 0;
}

static bool darwin_reserve_read_write(uintptr_t preferred_address, size_t size, void **mapping,
                                      ScoopPlatformError *error) {
    if (size == 0 || mapping == NULL || error == NULL) {
        return false;
    }
    void *reserved = mmap((void *)preferred_address, size, PROT_READ | PROT_WRITE,
                          MAP_PRIVATE | MAP_ANON, -1, 0);
    if (reserved == MAP_FAILED) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    *mapping = reserved;
    return true;
}

static bool darwin_protect_none(void *base, size_t size, ScoopPlatformError *error) {
    size_t page_size = darwin_page_size();
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

static bool darwin_release_mapping(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL) {
        return false;
    }
    if (base == NULL || size == 0 || munmap(base, size) != 0) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static bool darwin_discard_pages(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL) {
        return false;
    }
    if (base == NULL || size == 0 || madvise(base, size, MADV_DONTNEED) != 0) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static void darwin_resident_memory(uint64_t *current_bytes, uint64_t *peak_bytes) {
    *current_bytes = *peak_bytes = 0;
    mach_task_basic_info_data_t info;
    mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
    if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&info, &count) ==
        KERN_SUCCESS) {
        *current_bytes = info.resident_size;
    }
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage) == 0 && usage.ru_maxrss > 0) {
        *peak_bytes = (uint64_t)usage.ru_maxrss;
    }
}

const ScoopThreadVmOps scoop_darwin_thread_vm_ops = {
    .stack_bounds = darwin_stack_bounds,
    .reserve_read_write = darwin_reserve_read_write,
    .page_size = darwin_page_size,
    .protect_none = darwin_protect_none,
    .release_mapping = darwin_release_mapping,
    .discard_pages = darwin_discard_pages,
    .resident_memory = darwin_resident_memory,
};
