#include <stdlib.h>
#include <string.h>

#include "internal.h"

static int compare_address(const void *left, const void *right) {
    uintptr_t a = (uintptr_t)((const ScoopContextCell *)left)->use->slot_cell;
    uintptr_t b = (uintptr_t)((const ScoopContextCell *)right)->use->slot_cell;
    return (a > b) - (a < b);
}

static int compare_key(const void *left, const void *right) {
    const ScoopContextCell *a = left;
    const ScoopContextCell *b = right;
    return memcmp(a->use->exact_key.bytes, b->use->exact_key.bytes, 32);
}

void scoop_image_context_keys(ScoopImageRegistry *registry) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_CALLABLE];
    size_t count = 0;
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_CALLABLE, index);
        const ScoopCallableRegistrationDescriptorV1 *body =
            table->entries[index].record;
        if (body->context_key_count == 0) {
            if (body->context_keys != NULL) {
                scoop_metadata_fatal(&check, "non-null empty Context key table");
            }
            continue;
        }
        scoop_metadata_readonly(&check, body->context_keys, body->context_key_count,
                                sizeof *body->context_keys, 8,
                                "Context key table range");
        if (body->context_key_count > SIZE_MAX - count) {
            scoop_metadata_fatal(&check, "Context key table size overflow");
        }
        count += (size_t)body->context_key_count;
    }
    registry->context_cells =
        scoop_metadata_allocate(count, sizeof *registry->context_cells);
    registry->context_cell_count = count;
    size_t next = 0;
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_CALLABLE, index);
        const ScoopCallableRegistrationDescriptorV1 *body =
            table->entries[index].record;
        for (uint64_t key = 0; key < body->context_key_count; key++) {
            const ScoopContextKeyUseV1 *use = &body->context_keys[key];
            scoop_metadata_writable(&check, use->slot_cell, sizeof *use->slot_cell, 8,
                                    "Context slot cell range");
            if (*use->slot_cell != 0 || scoop_digest_zero(&use->exact_key)) {
                scoop_metadata_fatal(&check, "Context cell initial value or exact key");
            }
            if (key != 0 && memcmp(body->context_keys[key - 1].exact_key.bytes,
                                   use->exact_key.bytes, 32) >= 0) {
                scoop_metadata_fatal(&check,
                                     "Context keys are not distinct and ordered");
            }
            registry->context_cells[next++] = (ScoopContextCell){use, index, 0};
        }
    }
    qsort(registry->context_cells, count, sizeof *registry->context_cells,
          compare_address);
    for (size_t index = 1; index < count; index++) {
        if (compare_address(&registry->context_cells[index - 1],
                            &registry->context_cells[index]) == 0) {
            ScoopMetadataCheck check = scoop_record_check(
                registry, SCOOP_RECORD_CALLABLE, registry->context_cells[index].owner);
            scoop_metadata_fatal(&check,
                                 "Context cell shared by different callable keys");
        }
    }
    qsort(registry->context_cells, count, sizeof *registry->context_cells, compare_key);
    uint64_t distinct = 0;
    for (size_t index = 0; index < count; index++) {
        if (index == 0 || compare_key(&registry->context_cells[index - 1],
                                      &registry->context_cells[index]) != 0) {
            distinct++;
            if (distinct > (UINT64_C(1) << 32)) {
                scoop_metadata_fatal(
                    NULL, "Context key count exceeds the u32 slot representation");
            }
        }
        registry->context_cells[index].encoded_slot = distinct;
    }
    registry->context_key_count = distinct;
    uint64_t capacity = 1;
    while (capacity < distinct || (distinct != 0 && registry->context_height == 0)) {
        capacity *= 4;
        registry->context_height++;
    }
}
