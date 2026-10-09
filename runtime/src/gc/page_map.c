/* Sparse address lookup shared by generated stores and runtime object queries. */
#include <stdlib.h>

#include "heap_internal.h"
#include "../generated_entries.h"

typedef ScoopGcPageMapEntry PageMapEntry;
ScoopGcPageMapEntry scoop_gc_page_map[GC_RADIX_ENTRIES];

_Static_assert(ATOMIC_POINTER_LOCK_FREE == 2, "page map reads must not acquire a lock");

static size_t radix_index(uintptr_t address, unsigned shift) {
    return (address >> shift) & (GC_RADIX_ENTRIES - 1);
}

ScoopGcRegion *scoop_heap_region_for_address(uintptr_t address) {
    PageMapEntry *node = scoop_gc_page_map;
    for (unsigned shift = 52; shift > GC_CHUNK_SHIFT; shift -= GC_RADIX_BITS) {
        node = atomic_load_explicit(&node[radix_index(address, shift)], memory_order_acquire);
        if (node == NULL) {
            return NULL;
        }
    }
    return atomic_load_explicit(&node[radix_index(address, GC_CHUNK_SHIFT)], memory_order_acquire);
}

static PageMapEntry *leaf_for_publication(uintptr_t address) {
    PageMapEntry *node = scoop_gc_page_map;
    for (unsigned shift = 52; shift > GC_CHUNK_SHIFT; shift -= GC_RADIX_BITS) {
        PageMapEntry *slot = &node[radix_index(address, shift)];
        PageMapEntry *child = atomic_load_explicit(slot, memory_order_relaxed);
        if (child == NULL) {
            child = calloc(GC_RADIX_ENTRIES, sizeof *child);
            if (child == NULL) {
                heap_fatal("out of memory allocating the heap page map");
            }
            atomic_store_explicit(slot, child, memory_order_release);
        }
        node = child;
    }
    return &node[radix_index(address, GC_CHUNK_SHIFT)];
}

void scoop_heap_page_map_publish(ScoopGcRegion *region) {
    for (size_t offset = 0; offset < region->size; offset += GC_CHUNK_SIZE) {
        PageMapEntry *slot = leaf_for_publication(region->base + offset);
        if (atomic_load_explicit(slot, memory_order_relaxed) != NULL) {
            heap_fatal("overlapping heap mappings");
        }
        atomic_store_explicit(slot, region, memory_order_release);
    }
}

void scoop_heap_page_map_remove(const ScoopGcRegion *region) {
    for (size_t offset = 0; offset < region->size; offset += GC_CHUNK_SIZE) {
        uintptr_t address = region->base + offset;
        PageMapEntry *node = scoop_gc_page_map;
        for (unsigned shift = 52; shift > GC_CHUNK_SHIFT; shift -= GC_RADIX_BITS) {
            node = atomic_load_explicit(&node[radix_index(address, shift)], memory_order_relaxed);
        }
        atomic_store_explicit(&node[radix_index(address, GC_CHUNK_SHIFT)], NULL,
                              memory_order_release);
    }
}

static bool prune_node(PageMapEntry *node, unsigned depth) {
    bool empty = true;
    for (size_t index = 0; index < GC_RADIX_ENTRIES; index++) {
        void *child = atomic_load_explicit(&node[index], memory_order_relaxed);
        if (child == NULL) {
            continue;
        }
        if (depth != 0 && prune_node(child, depth - 1)) {
            atomic_store_explicit(&node[index], NULL, memory_order_release);
            free(child);
        } else {
            empty = false;
        }
    }
    return empty;
}

void scoop_heap_page_map_prune(void) { (void)prune_node(scoop_gc_page_map, 3); }
