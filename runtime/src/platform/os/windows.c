#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <psapi.h>

#include "../platform.h"

#if !defined(_WIN64)
#error "the Windows OS component requires 64-bit Windows"
#endif

static bool windows_stack_bounds(ScoopPlatformStackBounds *bounds, ScoopPlatformError *error) {
    if (bounds == NULL || error == NULL)
        return false;
    ULONG_PTR low, high;
    GetCurrentThreadStackLimits(&low, &high);
    if (low == 0 || high <= low) {
        error->code = SCOOP_PLATFORM_INVALID_STACK_BOUNDS;
        return false;
    }
    *bounds = (ScoopPlatformStackBounds){(const char *)low, (const char *)high};
    return true;
}

static size_t windows_page_size(void) {
    SYSTEM_INFO info;
    GetSystemInfo(&info);
    return info.dwPageSize;
}

static size_t windows_processor_count(void) {
    DWORD count = GetActiveProcessorCount(ALL_PROCESSOR_GROUPS);
    return count != 0 ? count : 1;
}

static bool windows_reserve_read_write(size_t size, size_t alignment, void **mapping,
                                       ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    SYSTEM_INFO info;
    GetSystemInfo(&info);
    size_t page = info.dwPageSize, granularity = info.dwAllocationGranularity;
    size_t padding = alignment > granularity ? alignment - granularity : 0;
    if (mapping == NULL || page == 0 || size == 0 || size % page != 0 || alignment < page ||
        alignment % page != 0 || (alignment & (alignment - 1)) != 0 || size > SIZE_MAX - padding) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    void *reserved = VirtualAlloc(NULL, size + padding, MEM_RESERVE, PAGE_NOACCESS);
    if (reserved == NULL) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    uintptr_t aligned = ((uintptr_t)reserved + alignment - 1) & ~(uintptr_t)(alignment - 1);
    void *committed = VirtualAlloc((void *)aligned, size, MEM_COMMIT, PAGE_READWRITE);
    if (committed == NULL) {
        VirtualFree(reserved, 0, MEM_RELEASE);
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    *mapping = committed;
    return true;
}

static bool page_range(void *base, size_t size) {
    size_t page = windows_page_size();
    return base != NULL && page != 0 && size != 0 && (uintptr_t)base % page == 0 &&
           size % page == 0 && size <= UINTPTR_MAX - (uintptr_t)base;
}

static bool windows_protect_none(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    DWORD previous;
    if (!page_range(base, size) || !VirtualProtect(base, size, PAGE_NOACCESS, &previous)) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static bool windows_release_mapping(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    MEMORY_BASIC_INFORMATION info;
    /* Alignment padding belongs to the original reservation, not the GC range. */
    if (!page_range(base, size) || VirtualQuery(base, &info, sizeof info) != sizeof info ||
        !VirtualFree(info.AllocationBase, 0, MEM_RELEASE)) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static bool windows_discard_pages(void *base, size_t size, ScoopPlatformError *error) {
    if (error == NULL)
        return false;
    if (!page_range(base, size) || DiscardVirtualMemory(base, size) != ERROR_SUCCESS) {
        error->code = SCOOP_PLATFORM_VM_OPERATION_FAILED;
        return false;
    }
    return true;
}

static void windows_resident_memory(uint64_t *current_bytes, uint64_t *peak_bytes) {
    *current_bytes = *peak_bytes = 0;
    PROCESS_MEMORY_COUNTERS counters = {.cb = sizeof counters};
    if (GetProcessMemoryInfo(GetCurrentProcess(), &counters, sizeof counters)) {
        *current_bytes = counters.WorkingSetSize;
        *peak_bytes = counters.PeakWorkingSetSize;
    }
}

const ScoopThreadVmOps scoop_windows_thread_vm_ops = {
    .stack_bounds = windows_stack_bounds,
    .reserve_read_write = windows_reserve_read_write,
    .page_size = windows_page_size,
    .processor_count = windows_processor_count,
    .protect_none = windows_protect_none,
    .release_mapping = windows_release_mapping,
    .discard_pages = windows_discard_pages,
    .resident_memory = windows_resident_memory,
};
