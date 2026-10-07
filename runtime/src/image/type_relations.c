#include <stdlib.h>
#include <string.h>

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
    if (td->relation_kind > 4 ||
        ((td->relation_kind == 0 || td->relation_kind == 4) && td->related_type_count != 0) ||
        ((td->relation_kind == 0 || td->relation_kind == 3 || td->relation_kind == 4) &&
         td->function_result != NULL)) {
        scoop_metadata_fatal(&check, "relation kind, count or function result");
    }
    if (td->relation_kind == 4 &&
        (td->instance_shape.instance_kind != SCOOP_TYPE_INSTANCE_ABSTRACT_REF_V1 ||
         td->parent != NULL || td->itable_count != 0)) {
        scoop_metadata_fatal(&check, "bottom type must be an abstract reference without parent or itables");
    }
    if (td->parent != NULL) {
        registered_type(registry, &check, td->parent);
    }
    if (td->function_result != NULL) {
        registered_type(registry, &check, td->function_result);
    }
    const ScoopRegistrationIdentityV1 *previous = NULL;
    for (size_t related = 0; related < td->related_type_count; related++) {
        const ScoopTypeDescriptor *target = td->related_types[related];
        if (target == NULL && td->relation_kind != 3) {
            continue; /* Any in a function signature. */
        }
        size_t target_index = registered_type(registry, &check, target);
        if (td->relation_kind == 3) {
            const ScoopRegistrationIdentityV1 *identity =
                types->entries[target_index].identity;
            if (target->relation_kind != 3 ||
                (previous != NULL && memcmp(previous->semantic_id.bytes,
                                            identity->semantic_id.bytes, 32) >= 0)) {
                scoop_metadata_fatal(&check, "interface parents or canonical order");
            }
            previous = identity;
        }
    }
    if (td->vtable != NULL) {
        /* Slot count and ABI belong to compiler/reader validation. */
        scoop_metadata_readonly(&check, td->vtable, 0, sizeof(void *), 8,
                                "vtable range");
    }
    if (td->itables != NULL || td->itable_count != 0) {
        scoop_metadata_readonly(&check, td->itables, td->itable_count,
                                sizeof *td->itables, 8, "itable entries range");
    }
    for (size_t slot = 0; slot < td->itable_count; slot++) {
        const ScoopItableEntryV1 *entry = &td->itables[slot];
        registered_type(registry, &check, entry->interface);
        if (entry->interface->relation_kind != 3) {
            scoop_metadata_fatal(&check, "itable key is not an interface");
        }
        for (size_t previous_slot = 0; previous_slot < slot; previous_slot++) {
            if (td->itables[previous_slot].interface == entry->interface) {
                scoop_metadata_fatal(&check, "duplicate itable key");
            }
        }
        if (entry->slots != NULL) {
            scoop_metadata_readonly(&check, entry->slots, 0, sizeof(void *), 8,
                                    "itable slots range");
        }
    }
}

typedef struct InheritanceVisit {
    size_t next_edge;
    size_t parent;
    bool active;
    bool complete;
} InheritanceVisit;

static void check_inheritance(const ScoopImageRegistry *registry) {
    const ScoopRecordTable *types = &registry->tables[SCOOP_RECORD_TYPE];
    InheritanceVisit *visits = scoop_metadata_allocate(types->count, sizeof *visits);
    for (size_t start = 0; start < types->count; start++) {
        if (visits[start].complete) {
            continue;
        }
        visits[start].active = true;
        visits[start].parent = SIZE_MAX;
        size_t current = start;
        while (current != SIZE_MAX) {
            InheritanceVisit *visit = &visits[current];
            const ScoopTypeRegistrationDescriptorV1 *type =
                types->entries[current].record;
            const ScoopTypeDescriptor *td = type->descriptor;
            const ScoopTypeDescriptor *target = NULL;
            if (visit->next_edge == 0) {
                target = td->parent;
                visit->next_edge++;
            } else if (td->relation_kind == 3 &&
                       visit->next_edge <= td->related_type_count) {
                target = td->related_types[visit->next_edge++ - 1];
            } else {
                visit->active = false;
                visit->complete = true;
                current = visit->parent;
                continue;
            }
            if (target == NULL) {
                continue;
            }
            ScoopMetadataCheck check =
                scoop_record_check(registry, SCOOP_RECORD_TYPE, current);
            size_t child = registered_type(registry, &check, target);
            if (visits[child].active) {
                scoop_metadata_fatal(&check, "cyclic type inheritance");
            }
            if (!visits[child].complete) {
                visits[child].active = true;
                visits[child].parent = current;
                current = child;
            }
        }
    }
    free(visits);
}

void scoop_image_type_relations(const ScoopImageRegistry *registry) {
    for (size_t index = 0; index < registry->tables[SCOOP_RECORD_TYPE].count; index++) {
        check_relations(registry, index);
    }
    check_inheritance(registry);
}
