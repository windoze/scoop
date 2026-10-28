#include "platform/platform.h"
#include "platform/primitives.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

extern const ScoopThreadVmOps scoop_windows_thread_vm_ops;
static const ScoopThreadVmOps *const vm = &scoop_windows_thread_vm_ops;

__attribute__((noinline)) static void grow_stack(unsigned depth) {
    volatile unsigned char storage[8192];
    storage[0] = (unsigned char)depth;
    storage[sizeof storage - 1] = 75;
    ScoopPlatformStackBounds bounds;
    ScoopPlatformError error = {0};
    assert(vm->stack_bounds(&bounds, &error));
    assert((uintptr_t)storage >= (uintptr_t)bounds.low);
    assert((uintptr_t)storage + sizeof storage <= (uintptr_t)bounds.high);
    if (depth != 0)
        grow_stack(depth - 1);
    assert(storage[0] == depth && storage[sizeof storage - 1] == 75);
}

static void *worker(void *argument) {
    (void)argument;
    grow_stack(16);
    return NULL;
}

static MEMORY_BASIC_INFORMATION query(void *address) {
    MEMORY_BASIC_INFORMATION info;
    assert(VirtualQuery(address, &info, sizeof info) == sizeof info);
    return info;
}

static void mapping_case(size_t size, size_t alignment, size_t page) {
    ScoopPlatformError error = {0};
    void *mapping;
    assert(vm->reserve_read_write(size, alignment, &mapping, &error));
    assert((uintptr_t)mapping % alignment == 0);
    MEMORY_BASIC_INFORMATION info = query(mapping);
    assert(info.State == MEM_COMMIT && info.Protect == PAGE_READWRITE && info.RegionSize == size);
    void *reservation = info.AllocationBase;
    if (reservation != mapping)
        assert(query(reservation).State == MEM_RESERVE);
    unsigned char *bytes = mapping;
    for (size_t offset = 0; offset < size; offset += page)
        assert(bytes[offset] == 0);
    memset(mapping, 93, size);
    assert(vm->discard_pages(bytes + page, size - 2 * page, &error));
    for (size_t offset = 0; offset < page; ++offset) {
        assert(bytes[offset] == 93);
        assert(bytes[size - page + offset] == 93);
    }
    memset(bytes + page, 42, size - 2 * page);
    assert(bytes[page] == 42 && bytes[size - page - 1] == 42);
    assert(query(mapping).State == MEM_COMMIT);
    assert(!vm->discard_pages(bytes + 1, page, &error));
    assert(!vm->protect_none(bytes + 1, page, &error));
    assert(vm->protect_none(bytes + page, page, &error));
    assert(query(bytes + page).Protect == PAGE_NOACCESS);
    assert(!vm->discard_pages(bytes + page, page, &error));
    assert(bytes[0] == 93 && bytes[size - 1] == 93);
    assert(vm->release_mapping(mapping, size, &error));
    assert(query(reservation).State == MEM_FREE);
    assert(query(bytes + size - 1).State == MEM_FREE);
}

int wmain(int argc, uint16_t **argv) {
    (void)argc;
    (void)argv;
    grow_stack(16);
    ScoopPlatformThread thread;
    assert(scoop_runtime_platform_thread_create(&thread, worker, NULL) == 0);
    assert(scoop_runtime_platform_thread_join(thread) == 0);
    size_t page = vm->page_size();
    assert(page >= 4096 && vm->processor_count() != 0);
    ScoopPlatformError error = {0};
    void *mapping = NULL;
    assert(!vm->reserve_read_write(0, page, &mapping, &error));
    assert(!vm->reserve_read_write(page, page * 3, &mapping, &error));
    assert(!vm->reserve_read_write(SIZE_MAX, page, &mapping, &error));
    for (unsigned iteration = 0; iteration < 4; ++iteration) {
        mapping_case(page * 3, page, page);
        mapping_case(128 * 1024, 64 * 1024, page);
        mapping_case(16 * 1024 * 1024, 16 * 1024 * 1024, page);
    }
    uint64_t current, peak;
    vm->resident_memory(&current, &peak);
    assert(current != 0 && peak >= current);
    puts("aligned regions, whole reservations, discard/reuse, guards, stacks and RSS passed");
    return 0;
}
