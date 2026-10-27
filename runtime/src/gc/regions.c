/* Aligned mappings retain an owner pointer in their reserved first line. */
#include <stdlib.h>

#include "heap_internal.h"
#include "../platform/platform.h"

static void release_mapping(void *base, size_t size) {
    ScoopPlatformError error = {0};
    if (!scoop_platform_bundle()->thread_vm->release_mapping(base, size, &error)) {
        heap_fatal(scoop_platform_error_message(error.code));
    }
}

size_t scoop_heap_large_mapping_size(size_t exact_size) {
    if (exact_size > SIZE_MAX - GC_LINE_SIZE - (GC_CHUNK_SIZE - 1)) {
        heap_fatal("large object mapping size overflows");
    }
    return (GC_LINE_SIZE + exact_size + GC_CHUNK_SIZE - 1) & ~(GC_CHUNK_SIZE - 1);
}

static void *map_aligned(size_t size, size_t alignment) {
    const ScoopThreadVmOps *vm = scoop_platform_bundle()->thread_vm;
    size_t page_size = vm->page_size();
    if (page_size == 0 || alignment % page_size != 0 || size > SIZE_MAX - alignment) {
        heap_fatal("heap mapping cannot satisfy platform alignment");
    }
    size_t reserved_size = size + alignment;
    void *mapping = NULL;
    ScoopPlatformError error = {0};
    if (!vm->reserve_read_write(0, reserved_size, &mapping, &error)) {
        return NULL;
    }
    uintptr_t raw = (uintptr_t)mapping;
    if (raw > UINTPTR_MAX - reserved_size) {
        release_mapping(mapping, reserved_size);
        return NULL;
    }
    uintptr_t aligned = (raw + alignment - 1) & ~(uintptr_t)(alignment - 1);
    size_t prefix = aligned - raw;
    size_t suffix = reserved_size - prefix - size;
    if (prefix != 0) {
        release_mapping(mapping, prefix);
    }
    if (suffix != 0) {
        release_mapping((void *)(aligned + size), suffix);
    }
    return (void *)aligned;
}

ScoopGcRegion *scoop_heap_region_create(size_t size, bool large) {
    void *mapping = map_aligned(size, large ? GC_CHUNK_SIZE : GC_REGION_SIZE);
    if (mapping == NULL) {
        return NULL;
    }
    ScoopGcRegion *region = calloc(1, sizeof *region);
    if (region == NULL) {
        heap_fatal("out of memory allocating region metadata");
    }
    region->base = (uintptr_t)mapping;
    region->size = size;
    region->large = large;
    region->cards = calloc(size >> GC_CARD_SHIFT, 1);
    size_t group_count = size >> (GC_CARD_SHIFT + GC_CARD_GROUP_SHIFT);
    region->card_groups = calloc(group_count, sizeof *region->card_groups);
    size_t block_count = large ? 1 : GC_REGION_BLOCKS;
    region->blocks = calloc(block_count, sizeof *region->blocks);
    if (region->cards == NULL || region->card_groups == NULL || region->blocks == NULL) {
        heap_fatal("out of memory allocating region side tables");
    }
    for (size_t index = 0; index < group_count; index++) {
        atomic_init(&region->card_groups[index].next, NULL);
        region->card_groups[index].region = region;
    }
    for (size_t index = 0; index < block_count; index++) {
        region->blocks[index].region = region;
        region->blocks[index].index = (uint16_t)index;
    }
    *(ScoopGcRegion **)mapping = region;
    region->next = scoop_gc_heap_state.regions;
    scoop_gc_heap_state.regions = region;
    if (!large) {
#if SCOOP_ENABLE_GC_STRESS
        /* Keep the mapping owner readable even when old object blocks are
         * quarantined. The reserved block never acquires object metadata. */
        region->next_block = 1;
#endif
        region->next_available = scoop_gc_heap_state.available_regions;
        scoop_gc_heap_state.available_regions = region;
    }
    scoop_heap_page_map_publish(region);
    scoop_gc_heap_state.metrics.region_mappings++;
    return region;
}

void scoop_heap_region_destroy(ScoopGcRegion *region) {
    scoop_heap_page_map_remove(region);
    release_mapping((void *)region->base, region->size);
    scoop_gc_heap_state.metrics.unmapped_bytes += region->size;
    size_t count = region->large ? 1 : GC_REGION_BLOCKS;
    for (size_t index = 0; index < count; index++) {
        ScoopGcBlockMeta *block = &region->blocks[index];
        free(block->starts);
        free(block->marks);
        free(block->pins);
#if SCOOP_GC_TRACK_RELOCATION
        free(block->scanned);
#endif
        free(block->line_live);
    }
    free(region->blocks);
    free(region->cards);
    free(region->card_groups);
    free(region);
}
