/* NoGC range barrier, shared by native and generated aggregate stores. */
#include "heap_internal.h"

void scoop_rt_gc_write_barrier(const void *destination, size_t bytes) {
    uintptr_t address = (uintptr_t)destination;
    if (bytes > UINTPTR_MAX - address) {
        heap_fatal("write barrier range overflows");
    }
    while (bytes != 0) {
        ScoopGcRegion *region = scoop_heap_region_for_address(address);
        if (region == NULL) {
            heap_fatal("write barrier range lies outside heap mappings");
        }
        size_t offset = address - region->base;
        size_t part = region->size - offset;
        if (part > bytes) {
            part = bytes;
        }
        size_t first = offset >> GC_CARD_SHIFT;
        size_t last = (offset + part - 1) >> GC_CARD_SHIFT;
        for (size_t card = first; card <= last; card++) {
            (void)__atomic_fetch_or(&region->cards[card], 1, __ATOMIC_RELAXED);
        }
        address += part;
        bytes -= part;
    }
}
