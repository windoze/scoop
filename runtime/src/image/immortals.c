#include <stdlib.h>

#include "../value_shape.h"
#include "internal.h"

const ScoopImmortalObjectDescriptorV1 *
scoop_image_immortal(const ScoopImageRegistry *registry, const void *object) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_IMMORTAL];
    size_t index = scoop_record_address_find(registry->immortal_addresses,
                                             table->count, (uintptr_t)object);
    return index == SIZE_MAX ? NULL : table->entries[index].record;
}

void scoop_image_immortals(ScoopImageRegistry *registry) {
    const ScoopRecordTable *table = &registry->tables[SCOOP_RECORD_IMMORTAL];
    registry->immortal_addresses = scoop_metadata_allocate(
        table->count, sizeof *registry->immortal_addresses);
    for (size_t index = 0; index < table->count; index++) {
        ScoopMetadataCheck check =
            scoop_record_check(registry, SCOOP_RECORD_IMMORTAL, index);
        const ScoopImmortalObjectDescriptorV1 *object =
            table->entries[index].record;
        const ScoopRegisteredRecord *type = scoop_record_by_address(
            registry, SCOOP_RECORD_TYPE, object->type_registration);
        if (type == NULL) {
            scoop_metadata_fatal(&check, "unregistered type registration");
        }
        const ScoopTypeDescriptor *td = object->type_registration->descriptor;
        if (object->object_size < sizeof(ScoopObjectHeader) ||
            object->required_alignment !=
                td->instance_shape.instance_alignment ||
            td->object_scan != NULL) {
            scoop_metadata_fatal(&check,
                                 "object size, alignment or reference scan");
        }
        scoop_metadata_readonly(
            &check, object->object_start, object->object_size, 1,
            (size_t)object->required_alignment, "object readonly range");
        const ScoopObjectHeader *header = object->object_start;
        if (header->td != td || header->gc_word != 0) {
            scoop_metadata_fatal(&check,
                                 "object header TypeDescriptor or GC word");
        }
        scoop_shape_validate_object(object->object_start,
                                    (size_t)object->object_size);
        registry->immortal_addresses[index] =
            (ScoopRecordAddress){(uintptr_t)object->object_start, index};
    }
    qsort(registry->immortal_addresses, table->count,
          sizeof *registry->immortal_addresses, scoop_record_address_compare);
    for (size_t index = 1; index < table->count; index++) {
        const ScoopImmortalObjectDescriptorV1 *previous =
            table->entries[registry->immortal_addresses[index - 1].index]
                .record;
        if ((uintptr_t)previous->object_start + previous->object_size >
            registry->immortal_addresses[index].address) {
            ScoopMetadataCheck check =
                scoop_record_check(registry, SCOOP_RECORD_IMMORTAL,
                                   registry->immortal_addresses[index].index);
            scoop_metadata_fatal(&check, "overlapping immortal ranges");
        }
    }
}
