#include <stdlib.h>

#include "internal.h"

typedef struct AllocationRange {
    uintptr_t start, end;
    ScoopRecordKind kind;
    size_t index;
} AllocationRange;

static int range_order(const void *left, const void *right) {
    uintptr_t a = ((const AllocationRange *)left)->start;
    uintptr_t b = ((const AllocationRange *)right)->start;
    return (a > b) - (a < b);
}

void scoop_image_allocation_ranges(const ScoopImageRegistry *registry) {
    const ScoopRecordTable *storages = &registry->tables[SCOOP_RECORD_STORAGE];
    const ScoopRecordTable *units = &registry->tables[SCOOP_RECORD_UNIT];
    if (units->count > SIZE_MAX - storages->count) {
        scoop_metadata_fatal(NULL, "allocation range count overflow");
    }
    size_t count = storages->count + units->count;
    AllocationRange *ranges = scoop_metadata_allocate(count, sizeof *ranges);
    for (size_t index = 0; index < storages->count; index++) {
        const ScoopStaticStorageDescriptorV1 *storage =
            storages->entries[index].record;
        uintptr_t start = (uintptr_t)storage->writable_base;
        ranges[index] =
            (AllocationRange){start, start + storage->allocation_extent,
                              SCOOP_RECORD_STORAGE, index};
    }
    for (size_t index = 0; index < units->count; index++) {
        const ScoopInitializationUnitDescriptorV1 *unit =
            units->entries[index].record;
        uintptr_t start = (uintptr_t)unit->cell;
        ranges[storages->count + index] = (AllocationRange){
            start, start + sizeof *unit->cell, SCOOP_RECORD_UNIT, index};
    }
    qsort(ranges, count, sizeof *ranges, range_order);
    for (size_t index = 1; index < count; index++) {
        if (ranges[index - 1].end > ranges[index].start) {
            ScoopMetadataCheck check = scoop_record_check(
                registry, ranges[index].kind, ranges[index].index);
            scoop_metadata_fatal(
                &check, "overlapping storage or cell allocation ranges");
        }
    }
    free(ranges);
}
