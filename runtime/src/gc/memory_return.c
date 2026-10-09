/* Return unused physical pages and empty mappings after all reference updates. */
#include "heap_internal.h"
#include "../platform/platform.h"

bool scoop_heap_region_empty(const ScoopGcRegion *region) {
    for (size_t index = 0; index < region->next_block; index++) {
        ScoopGcBlockState state = region->blocks[index].state;
        if (state != SCOOP_BLOCK_FREE && state != SCOOP_BLOCK_NEVER_USED) {
            return false;
        }
    }
    return true;
}

static bool discard_range(const ScoopThreadVmOps *vm, uintptr_t begin, uintptr_t end) {
    if (begin == end) {
        return true;
    }
    ScoopPlatformError error = {0};
    if (!vm->discard_pages((void *)begin, end - begin, &error)) {
        scoop_gc_heap_state.metrics.discard_failures++;
        return false;
    }
    scoop_gc_heap_state.metrics.discard_calls++;
    scoop_gc_heap_state.metrics.discarded_bytes += end - begin;
    return true;
}

static bool empty_page(const ScoopGcBlockMeta *block, size_t offset, size_t page_size) {
    if (!block->discard_pending || block->state == SCOOP_BLOCK_QUARANTINED) {
        return false;
    }
    if (!active_head(block)) {
        return true;
    }
    for (size_t line = offset / GC_LINE_SIZE; line < (offset + page_size) / GC_LINE_SIZE; line++) {
        if (bit_test(block->line_occupied, line)) {
            return false;
        }
    }
    return true;
}

static void discard_empty_pages(ScoopGcRegion *region, const ScoopThreadVmOps *vm,
                                size_t page_size) {
    uintptr_t begin = region->base, end = begin;
    bool succeeded = true;
    for (size_t index = 0; index < region->next_block; index++) {
        ScoopGcBlockMeta *block = &region->blocks[index];
        for (size_t offset = 0; offset < GC_BLOCK_SIZE; offset += page_size) {
            uintptr_t address = region->base + index * GC_BLOCK_SIZE + offset;
            if (empty_page(block, offset, page_size)) {
                if (begin == end) {
                    begin = address;
                }
                end = address + page_size;
            } else {
                succeeded &= discard_range(vm, begin, end);
                begin = end = address + page_size;
            }
        }
    }
    succeeded &= discard_range(vm, begin, end);
    if (succeeded) {
        for (size_t index = 0; index < region->next_block; index++) {
            region->blocks[index].discard_pending = false;
        }
    }
}

void scoop_heap_reclaim_regions(bool full) {
    const ScoopThreadVmOps *vm = scoop_platform_bundle()->thread_vm;
    size_t page_size = full ? vm->page_size() : GC_BLOCK_SIZE;
    if (page_size == 0 || page_size % GC_LINE_SIZE != 0 || GC_BLOCK_SIZE % page_size != 0) {
        heap_fatal("GC blocks cannot be divided into platform pages");
    }
    if (full) {
        scoop_gc_heap_state.free_blocks = NULL;
        scoop_gc_heap_state.allocation_region = NULL;
    }
    ScoopGcRegion *retained = NULL;
    bool removed = false;
    ScoopGcRegion **link = &scoop_gc_heap_state.regions;
    while (*link != NULL) {
        ScoopGcRegion *region = *link;
        bool empty = region->large ? region->blocks[0].state == SCOOP_BLOCK_FREE
                                   : full && scoop_heap_region_empty(region);
        bool release = empty && (region->large || retained != NULL);
        if (release) {
            *link = region->next;
            scoop_heap_region_destroy(region);
            removed = true;
            continue;
        }
        if (full && !region->large) {
            if (empty) {
                retained = region;
            }
            discard_empty_pages(region, vm, page_size);
            for (size_t index = 0; index < region->next_block; index++) {
                ScoopGcBlockMeta *block = &region->blocks[index];
                if (block->state == SCOOP_BLOCK_FREE) {
                    block->next_free = scoop_gc_heap_state.free_blocks;
                    scoop_gc_heap_state.free_blocks = block;
                }
            }
        }
        link = &region->next;
    }
    if (full) {
        scoop_gc_heap_state.allocation_region = retained;
    }
    if (removed) {
        scoop_heap_page_map_prune();
    }
}
