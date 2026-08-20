#include <string.h>

#include <assert.h>
#include <stdio.h>

#include "../src/gc/heap_internal.h"
#include "../src/generated_entries.h"

static const uintptr_t addresses[] = {
    UINT64_C(0x0000000000000000), UINT64_C(0x000000001fff0000), UINT64_C(0x0000001000000000),
    UINT64_C(0x0000123456780000), UINT64_C(0x123456789abc0000), UINT64_C(0x8000000000000000),
    UINT64_C(0xffffffffffff0000),
};

int main(void) {
    // Synthetic address keys exercise every uintptr_t bit without dereferencing
    // an address outside the OS's user mapping range.
    ScoopGcRegion regions[sizeof addresses / sizeof addresses[0]];
    memset(regions, 0, sizeof regions);
    scoop_heap_lock();
    for (size_t index = 0; index < sizeof addresses / sizeof addresses[0]; index++) {
        regions[index].base = addresses[index];
        regions[index].size = GC_CHUNK_SIZE;
        scoop_heap_page_map_publish(&regions[index]);
    }
    for (size_t index = 0; index < sizeof addresses / sizeof addresses[0]; index++) {
        assert(scoop_heap_region_for_address(addresses[index]) == &regions[index]);
        assert(scoop_heap_region_for_address(addresses[index] + GC_CHUNK_SIZE - 1) ==
               &regions[index]);
    }
    for (size_t index = 0; index < sizeof addresses / sizeof addresses[0]; index += 2) {
        scoop_heap_page_map_remove(&regions[index]);
    }
    scoop_heap_page_map_prune();
    for (size_t index = 0; index < sizeof addresses / sizeof addresses[0]; index++) {
        assert(scoop_heap_region_for_address(addresses[index]) ==
               (index % 2 ? &regions[index] : NULL));
        if (index % 2) {
            scoop_heap_page_map_remove(&regions[index]);
        }
    }
    scoop_heap_page_map_prune();
    for (size_t index = 0; index < GC_RADIX_ENTRIES; index++) {
        assert(atomic_load_explicit(&scoop_gc_page_map[index], memory_order_relaxed) == NULL);
    }
    scoop_heap_page_map_publish(&regions[4]);
    assert(scoop_heap_region_for_address(addresses[4] + 99) == &regions[4]);
    scoop_heap_page_map_remove(&regions[4]);
    scoop_heap_page_map_prune();
    scoop_heap_unlock();
    puts("sparse page map covers 64-bit keys and removes empty paths");
}
