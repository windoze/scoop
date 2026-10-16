#include "internal.h"

static size_t registered_type(const ScoopImageRegistry *registry,
                              const ScoopMetadataCheck *check,
                              const ScoopTypeDescriptor *td) {
    size_t index = scoop_record_address_find(registry->type_addresses,
                                             registry->tables[SCOOP_RECORD_TYPE].count,
                                             (uintptr_t)td);
    if (index == SIZE_MAX) {
        scoop_metadata_fatal(check, "reference to an unregistered TypeDescriptor");
    }
    return index;
}

static void check_relations(const ScoopImageRegistry *registry, size_t index) {
    const ScoopRecordTable *types = &registry->tables[SCOOP_RECORD_TYPE];
    ScoopMetadataCheck check = scoop_record_check(registry, SCOOP_RECORD_TYPE, index);
    const ScoopTypeRegistrationDescriptorV1 *type = types->entries[index].record;
    const ScoopTypeDescriptor *td = type->descriptor;
    if (td->parent != NULL) {
        registered_type(registry, &check, td->parent);
    }
    if (td->function_result != NULL) {
        registered_type(registry, &check, td->function_result);
    }
    for (size_t related = 0; related < td->related_type_count; related++) {
        const ScoopTypeDescriptor *target = td->related_types[related];
        if (target != NULL || td->relation_kind == 3) {
            registered_type(registry, &check, target);
        }
    }
    if (td->vtable != NULL) {
        /* Slot count and ABI belong to compiler/reader validation. */
        scoop_metadata_range(&check, td->vtable, 0, sizeof(void *), 8, "vtable range");
    }
    if (td->itables != NULL || td->itable_count != 0) {
        scoop_metadata_range(&check, td->itables, td->itable_count, sizeof *td->itables, 8,
                             "itable entries range");
    }
    for (size_t slot = 0; slot < td->itable_count; slot++) {
        const ScoopItableEntryV1 *entry = &td->itables[slot];
        registered_type(registry, &check, entry->interface);
        if (entry->slots != NULL) {
            scoop_metadata_range(&check, entry->slots, 0, sizeof(void *), 8, "itable slots range");
        }
    }
}

void scoop_image_type_relations(const ScoopImageRegistry *registry) {
    for (size_t index = 0; index < registry->tables[SCOOP_RECORD_TYPE].count; index++) {
        check_relations(registry, index);
    }
}
