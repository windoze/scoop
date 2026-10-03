#include <stdlib.h>
#include <string.h>

#include "internal.h"

int scoop_record_address_compare(const void *left, const void *right) {
    uintptr_t a = ((const ScoopRecordAddress *)left)->address;
    uintptr_t b = ((const ScoopRecordAddress *)right)->address;
    return (a > b) - (a < b);
}

size_t scoop_record_address_find(const ScoopRecordAddress *addresses, size_t count,
                                 uintptr_t address) {
    ScoopRecordAddress key = {.address = address};
    const ScoopRecordAddress *found = bsearch(&key, addresses, count, sizeof *addresses,
                                              scoop_record_address_compare);
    return found == NULL ? SIZE_MAX : found->index;
}

static int compare_identity(const void *left, const void *right) {
    const ScoopDigest256V1 *identity = left;
    const ScoopRegisteredRecord *entry = right;
    return memcmp(identity->bytes, entry->identity->semantic_id.bytes, 32);
}

const ScoopRegisteredRecord *scoop_record_by_id(const ScoopImageRegistry *registry,
                                                ScoopRecordKind kind,
                                                const ScoopDigest256V1 *identity) {
    const ScoopRecordTable *table = &registry->tables[kind];
    return bsearch(identity, table->entries, table->count, sizeof *table->entries,
                   compare_identity);
}

const ScoopRegisteredRecord *scoop_record_by_address(const ScoopImageRegistry *registry,
                                                     ScoopRecordKind kind,
                                                     const void *record) {
    const ScoopRecordTable *table = &registry->tables[kind];
    size_t index =
        scoop_record_address_find(table->addresses, table->count, (uintptr_t)record);
    return index == SIZE_MAX ? NULL : &table->entries[index];
}

const ScoopTypeRegistrationDescriptorV1 *
scoop_image_type(const ScoopImageRegistry *registry, const ScoopTypeDescriptor *td) {
    const ScoopRecordTable *types = &registry->tables[SCOOP_RECORD_TYPE];
    size_t index = scoop_record_address_find(registry->type_addresses, types->count,
                                             (uintptr_t)td);
    return index == SIZE_MAX ? NULL : types->entries[index].record;
}

const ScoopImmortalObjectDescriptorV1 *
scoop_image_immortal(const ScoopImageRegistry *registry, const void *object) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_IMMORTAL];
    size_t index = scoop_record_address_find(registry->immortal_addresses, table->count,
                                             (uintptr_t)object);
    return index == SIZE_MAX ? NULL : table->entries[index].record;
}
